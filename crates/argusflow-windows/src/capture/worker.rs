//! 适配器 actor 公平处理输出、GPU 完成和按需读取；有限恢复不增殖线程。
use super::{backend::Shared, gpu::Graphics, output::Output, requests::Requests, topology};
use argusflow_capture_contracts::*;
use argusflow_core::FailureKind;
use std::{
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
use windows::Win32::Graphics::Dxgi::IDXGIAdapter1;

pub(super) fn run(
    adapter: IDXGIAdapter1,
    index: u32,
    slot: usize,
    shared: Arc<Shared>,
    config: BackendConfig,
) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        actor(adapter, index, slot, &shared, &config)
    }));
    if outcome.is_err() {
        // 顶层 panic 不被伪装为健康水位；所有工作线程协作停止。
        shared.stop.store(true, Ordering::Release);
        *shared.fatal.lock().unwrap_or_else(|p| p.into_inner()) = Some(CaptureError::new(
            FailureKind::Native,
            "capture_worker",
            "采样原生线程 panic",
        ));
    }
}
fn actor(
    adapter: IDXGIAdapter1,
    index: u32,
    slot: usize,
    shared: &Arc<Shared>,
    config: &BackendConfig,
) {
    let budget = match ByteBudget::new(config.gpu_bytes) {
        Ok(budget) => budget,
        Err(_) => return,
    };
    // 只更改本工作线程的 DPI 上下文，不修改宿主进程的全局设置。
    let _dpi = super::dpi::DpiScope::enter();
    let (sender, receiver) = mpsc::sync_channel(config.read_queue);
    let mut graphics: Option<Graphics> = None;
    let mut outputs: Vec<Output> = Vec::new();
    let mut known: Vec<SourceInfo> = Vec::new();
    let mut requests = Requests::new();
    let mut stats = CaptureStats::default();
    let mut generation = 1_u64;
    let mut recovery_start = Instant::now();
    let mut retry_at = Instant::now();
    let mut backoff = Duration::from_millis(50);
    let mut topology_at = Instant::now();
    let mut restart = 0;
    let mut round = 0_usize;
    let mut exhausted = false;
    let mut desktop = super::desktop::current();
    while !shared.stop.load(Ordering::Acquire) {
        // 压力恢复也会增加输出代际；全适配器重建必须越过已使用过的所有代际。
        generation = outputs
            .iter()
            .map(|output| output.info.generation)
            .max()
            .unwrap_or(generation)
            .max(generation);
        let new_restart = shared.restart.load(Ordering::Acquire);
        if new_restart != restart {
            restart = new_restart;
            reset(
                &mut outputs,
                &mut graphics,
                &mut requests,
                shared,
                GapReason::DeviceReset,
            );
            generation += 1;
            recovery_start = Instant::now();
            retry_at = Instant::now();
            backoff = Duration::from_millis(50);
            exhausted = false;
        }
        if topology_at.elapsed() >= Duration::from_millis(500) {
            topology_at = Instant::now();
            let current_desktop = super::desktop::current();
            if current_desktop != desktop {
                desktop = current_desktop;
                reset(
                    &mut outputs,
                    &mut graphics,
                    &mut requests,
                    shared,
                    GapReason::TopologyChanged,
                );
                generation += 1;
                recovery_start = Instant::now();
                retry_at = Instant::now();
                backoff = Duration::from_millis(50);
                exhausted = false;
            }
            if let Ok(specs) = topology::outputs(&adapter, index, generation) {
                let changed = specs.len() != known.len()
                    || specs.iter().any(|spec| {
                        !known.iter().any(|old| {
                            old.id == spec.info.id
                                && old.bounds == spec.info.bounds
                                && old.rotation == spec.info.rotation
                                && old.dpi == spec.info.dpi
                                && (old.state == SourceState::Unavailable)
                                    == (spec.info.state == SourceState::Unavailable)
                        })
                    });
                if changed {
                    for old in &known {
                        if !specs.iter().any(|spec| spec.info.id == old.id) {
                            let mut removed = old.clone();
                            removed.state = SourceState::Removed;
                            shared
                                .queue
                                .push(BackendEvent::Source(removed), shared.clock.now());
                        }
                    }
                    reset(
                        &mut outputs,
                        &mut graphics,
                        &mut requests,
                        shared,
                        GapReason::TopologyChanged,
                    );
                    generation += 1;
                    recovery_start = Instant::now();
                    retry_at = Instant::now();
                    backoff = Duration::from_millis(50);
                    exhausted = false;
                    known = specs.into_iter().map(|spec| spec.info).collect();
                }
            }
        }
        if graphics.is_none() {
            Requests::reject_waiting(&receiver, config.read_queue);
        }
        if graphics.is_none() && !exhausted && Instant::now() >= retry_at {
            let initialized = (|| {
                let graphics = Graphics::new(&adapter, budget.clone())?;
                let specs = topology::outputs(&adapter, index, generation)?;
                let mut active = Vec::new();
                known = specs.iter().map(|spec| spec.info.clone()).collect();
                for mut spec in specs {
                    if u64::from(spec.raw_width) * u64::from(spec.raw_height) * 8 + 64
                        > config.gpu_bytes as u64
                    {
                        spec.info.state = SourceState::Unavailable;
                        spec.info.failure = Some(CaptureError::new(
                            FailureKind::ResourceLimit,
                            "dxgi_baseline",
                            "GPU 预算不足以保存当前输出的基线和冻结版本",
                        ));
                    }
                    shared
                        .queue
                        .push(BackendEvent::Source(spec.info.clone()), shared.clock.now());
                    if spec.info.state == SourceState::Unavailable {
                        continue;
                    }
                    active.push(Output::new(
                        spec,
                        &graphics,
                        shared.clock,
                        sender.clone(),
                        shared.queue.clone(),
                    )?);
                }
                Ok::<_, CaptureError>((graphics, active))
            })();
            match initialized {
                Ok((device, active)) => {
                    graphics = Some(device);
                    outputs = active;
                }
                Err(error) => {
                    for info in &known {
                        let mut recovering = info.clone();
                        recovering.state = SourceState::Recovering;
                        recovering.failure = Some(error.clone());
                        shared
                            .queue
                            .push(BackendEvent::Source(recovering), shared.clock.now());
                    }
                    retry_at = Instant::now() + backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(1));
                    if recovery_start.elapsed() >= config.recovery_timeout {
                        if known.is_empty() {
                            *shared.fatal.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some(error.clone());
                        }
                        exhausted = true;
                        for info in &known {
                            let mut failed = info.clone();
                            failed.state = SourceState::Unavailable;
                            failed.failure = Some(error.clone());
                            shared
                                .queue
                                .push(BackendEvent::Source(failed), shared.clock.now());
                        }
                    }
                }
            }
        }
        let mut error = None;
        if let Some(device) = &graphics {
            if let Err(failure) = requests.poll(device, &shared.cpu, config, &mut stats) {
                error = Some(failure);
            }
            if requests.available()
                && outputs
                    .iter()
                    .filter(|output| output.pending.is_some() || output.baseline.is_some())
                    .count()
                    < config.max_in_flight
                && let Ok(request) = receiver.try_recv()
            {
                requests.start(request, device);
            }
            let count = outputs.len();
            for offset in 0..count {
                let position = (round + offset) % count;
                let pending = outputs
                    .iter()
                    .filter(|output| output.pending.is_some() || output.baseline.is_some())
                    .count()
                    + usize::from(!requests.available());
                let output = &mut outputs[position];
                let result = output.finish(device, config, &mut stats).and_then(|()| {
                    if pending < config.max_in_flight {
                        // 只有没有其他 GPU 工作时才等待一次，其余输出非阻塞轮询。
                        let wait = if offset == 0 && pending == 0 && requests.available() {
                            config.acquire_wait.as_millis() as u32
                        } else {
                            0
                        };
                        output.acquire(device, wait, &mut stats)
                    } else {
                        Ok(())
                    }
                });
                if let Err(failure) = result {
                    if failure.kind() == FailureKind::ResourceLimit {
                        requests.pool.clear();
                        output.pressure();
                        stats.gaps += 1;
                    } else {
                        error = Some(failure);
                        break;
                    }
                }
            }
            round = round.wrapping_add(1);
        }
        if let Some(error) = error {
            generation = outputs
                .iter()
                .map(|output| output.info.generation)
                .max()
                .unwrap_or(generation)
                .max(generation);
            reset(
                &mut outputs,
                &mut graphics,
                &mut requests,
                shared,
                super::gpu::recovery_reason(&error),
            );
            stats.gaps += 1;
            generation += 1;
            for info in &known {
                let mut failed = info.clone();
                failed.state = SourceState::Recovering;
                failed.failure = Some(error.clone());
                shared
                    .queue
                    .push(BackendEvent::Source(failed), shared.clock.now());
            }
            if recovery_start.elapsed() >= config.recovery_timeout {
                exhausted = true;
                for info in &known {
                    let mut failed = info.clone();
                    failed.state = SourceState::Unavailable;
                    failed.failure = Some(error.clone());
                    shared
                        .queue
                        .push(BackendEvent::Source(failed), shared.clock.now());
                }
            } else {
                retry_at = Instant::now() + backoff;
                backoff = (backoff * 2).min(Duration::from_secs(1));
            }
        } else if graphics.is_some()
            && outputs
                .iter()
                .all(|output| output.info.state == SourceState::Ready)
        {
            recovery_start = Instant::now();
            backoff = Duration::from_millis(50);
        }
        stats.gpu_bytes = budget.used();
        stats.gpu_peak_bytes = budget.peak();
        shared.stats.lock().unwrap_or_else(|p| p.into_inner())[slot] = stats.clone();
        // 没有待处理 GPU 工作时 Acquire 负责等待；其他情况下让出线程避免紧密轮询。
        std::thread::sleep(if graphics.is_none() {
            Duration::from_millis(10)
        } else if outputs.is_empty() && requests.available() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(1)
        });
    }
    for output in &mut outputs {
        output.revoke();
        let mut info = output.info.clone();
        info.state = SourceState::Stopped;
        shared
            .queue
            .push(BackendEvent::Source(info), shared.clock.now());
    }
}
fn reset(
    outputs: &mut Vec<Output>,
    graphics: &mut Option<Graphics>,
    requests: &mut Requests,
    shared: &Shared,
    reason: GapReason,
) {
    for output in outputs.iter_mut() {
        output.gap(reason);
        output.revoke();
        let mut info = output.info.clone();
        info.state = SourceState::Recovering;
        shared
            .queue
            .push(BackendEvent::Source(info), shared.clock.now());
    }
    outputs.clear();
    *requests = Requests::new();
    *graphics = None;
}
