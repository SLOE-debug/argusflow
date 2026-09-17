//! 每适配器一个限速 actor；显示器变化与设备错误有界重建，停止信号不依赖新帧。
use super::{output::Output, service::Shared};
use crate::capture::{gpu::Graphics, topology};
use argusflow_capture_contracts::*;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use windows::Win32::Graphics::Dxgi::IDXGIAdapter1;

pub(super) fn run(adapter: IDXGIAdapter1, index: u32, shared: &Shared, config: &FrameConfig) {
    let _dpi = crate::capture::dpi::DpiScope::enter();
    let mut generation = 0;
    let mut failures = 0;
    while !shared.stop.load(Ordering::Acquire) {
        generation += 1;
        let started = Instant::now();
        match session(&adapter, index, generation, shared, config) {
            Ok(()) => break,
            Err(error) => {
                if started.elapsed() > Duration::from_secs(10) {
                    failures = 0;
                }
                failures += 1;
                let mut store = shared.store.lock().unwrap_or_else(|p| p.into_inner());
                if let Ok(specs) = topology::outputs(&adapter, index, generation) {
                    for spec in specs {
                        let mut info = spec.info;
                        info.failure = Some(error.clone());
                        info.state = if failures >= 10 {
                            SourceState::Unavailable
                        } else {
                            SourceState::Recovering
                        };
                        store.source(info);
                    }
                }
                if store.sources.is_empty() && failures >= 10 {
                    store.failure = Some(error);
                }
                drop(store);
                if failures >= 10 {
                    break;
                }
                for _ in 0..50 {
                    if shared.stop.load(Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}
fn session(
    adapter: &IDXGIAdapter1,
    index: u32,
    generation: u64,
    shared: &Shared,
    config: &FrameConfig,
) -> CaptureResult<()> {
    let graphics = Graphics::new(adapter, ByteBudget::new(config.gpu_bytes)?)?;
    let specs = topology::outputs(adapter, index, generation)?;
    let expected: Vec<_> = specs
        .iter()
        .map(|s| (s.info.id, s.info.bounds, s.info.rotation, s.info.dpi))
        .collect();
    let mut outputs = Vec::new();
    for spec in specs {
        shared
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .source(spec.info.clone());
        if spec.info.state != SourceState::Unavailable {
            outputs.push(Output::new(spec, &graphics, config)?);
        }
    }
    let mut topology_at = Instant::now();
    let desktop = crate::capture::desktop::current();
    while !shared.stop.load(Ordering::Acquire) {
        for output in &mut outputs {
            output.poll(&graphics, shared, config)?;
        }
        if topology_at.elapsed() >= Duration::from_millis(500) {
            topology_at = Instant::now();
            let specs = topology::outputs(adapter, index, generation)?;
            let actual: Vec<_> = specs
                .iter()
                .map(|s| (s.info.id, s.info.bounds, s.info.rotation, s.info.dpi))
                .collect();
            if expected != actual || desktop != crate::capture::desktop::current() {
                let mut store = shared.store.lock().unwrap_or_else(|p| p.into_inner());
                for output in &outputs {
                    let mut info = output.info.clone();
                    info.state = SourceState::Removed;
                    store.source(info);
                }
                return Err(CaptureError::new(
                    argusflow_core::FailureKind::Unavailable,
                    "frame_topology",
                    "显示来源发生变化",
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
