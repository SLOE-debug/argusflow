//! 应用级桌面采集主机；订阅与原生资源生命周期分离。
use crate::{CaptureBroker, CaptureCursor, CaptureStream, FrameUpdate};
use argusflow_core::{
    CaptureDelivery, CaptureFailure, CaptureSourceId, capture::ScreenCaptureSource,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// 平台提供的可唤醒高精度调度边界，采集 crate 不依赖 Windows。
pub trait CaptureScheduler: Send + Sync {
    /// 等待下一个检查时刻，检查间隔不得超过 2ms。
    fn wait(&self) -> Result<(), CaptureFailure>;
    /// 输入或关闭操作唤醒等待，不执行采集。
    fn wake(&self);
}

#[derive(Default)]
struct HubState {
    streams: HashMap<CaptureSourceId, Arc<CaptureStream>>,
    incident: u64,
    failure: Option<CaptureFailure>,
    readback_bytes: u64,
    diff_us: u64,
    barriers: Vec<
        std::sync::mpsc::SyncSender<
            Result<HashMap<CaptureSourceId, argusflow_core::CaptureRevision>, CaptureFailure>,
        >,
    >,
}

/// 同一原生提供器只有一个推进线程，慢消费者只影响自己的游标。
pub struct DesktopCaptureHub {
    state: Arc<Mutex<HubState>>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    scheduler: Arc<dyn CaptureScheduler>,
}

impl DesktopCaptureHub {
    /// 完成全部来源基准后返回；主机由应用拥有直到退出。
    pub fn start(
        source: Arc<dyn ScreenCaptureSource>,
        scheduler: Arc<dyn CaptureScheduler>,
    ) -> Result<Arc<Self>, CaptureFailure> {
        let sources = source.sources().map_err(|_| CaptureFailure::Unavailable)?;
        if sources.is_empty() {
            return Err(CaptureFailure::Unavailable);
        }
        let state = Arc::new(Mutex::new(HubState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let background_state = state.clone();
        let background_stop = stop.clone();
        let background_scheduler = scheduler.clone();
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let (pump, batches) = crate::desktop_pump::FramePump::channel();
        let pump_state = state.clone();
        let pump_stop = stop.clone();
        let pump_thread = std::thread::Builder::new()
            .name("argusflow-desktop-readback".into())
            .spawn(move || {
                while !pump_stop.load(Ordering::Acquire) {
                    let draining = pump_state
                        .lock()
                        .is_ok_and(|state| !state.barriers.is_empty());
                    if let Err(reason) = pump.poll(source.as_ref(), draining) {
                        incident(&pump_state, reason);
                        if reason == CaptureFailure::Closed {
                            break;
                        }
                        // 过载撤销有序订阅，但主机继续服务最新帧消费者；缺失增量后必须重新建立基准。
                        let _ = source.sources();
                    }
                    if let Err(reason) = background_scheduler.wait() {
                        incident(&pump_state, reason);
                        break;
                    }
                }
            })
            .map_err(|_| CaptureFailure::Unavailable)?;
        let pump_thread =
            crate::desktop_pump::PumpThread::new(stop.clone(), scheduler.clone(), pump_thread);
        let thread = std::thread::Builder::new()
            .name("argusflow-desktop-capture".into())
            .spawn(move || {
                let mut broker = CaptureBroker::default();
                let deadline = Instant::now() + Duration::from_secs(3);
                let mut initialized = false;
                while !background_stop.load(Ordering::Acquire) {
                    let mut drained = false;
                    let batch = match batches.recv_timeout(Duration::from_millis(2)) {
                        Ok(batch) => Some(batch),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    match batch {
                        Some(Ok(mut batch)) => {
                            drained = batch.drained;
                            for update in std::mem::take(&mut batch.updates) {
                                let bytes = update
                                    .patches
                                    .iter()
                                    .map(|patch| patch.pixels().len() as u64)
                                    .sum::<u64>();
                                let started = Instant::now();
                                if !update.reset && update.accumulated_frames > 1 {
                                    incident(&background_state, CaptureFailure::AccumulatedFrames);
                                }
                                match broker.publish_candidates(update) {
                                    Ok(Some(frame)) => {
                                        if let (Some(stream), Ok(mut state)) =
                                            (broker.source(frame.source), background_state.lock())
                                        {
                                            state.streams.insert(frame.source, stream);
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(reason) => incident(&background_state, reason),
                                }
                                if let Ok(mut state) = background_state.lock() {
                                    state.readback_bytes =
                                        state.readback_bytes.saturating_add(bytes);
                                    state.diff_us = state
                                        .diff_us
                                        .saturating_add(started.elapsed().as_micros() as u64);
                                }
                            }
                        }
                        Some(Err(_)) => {
                            incident(&background_state, CaptureFailure::Unavailable);
                            let _ = broker.close();
                            broker = CaptureBroker::default();
                            if let Ok(mut state) = background_state.lock() {
                                state.streams.clear();
                            }
                            // 提供器撤销失效设备后可重建；有序订阅看到事件立即终止自己的归档。
                            std::thread::park_timeout(Duration::from_millis(100));
                        }
                        None => {}
                    }
                    if drained {
                        if let Ok(mut state) = background_state.lock() {
                            let cutoff = state
                                .streams
                                .iter()
                                .map(|(source, stream)| {
                                    stream.latest().and_then(|frame| {
                                        frame
                                            .map(|frame| (*source, frame.revision))
                                            .ok_or(CaptureFailure::Unavailable)
                                    })
                                })
                                .collect::<Result<HashMap<_, _>, _>>();
                            for reply in state.barriers.drain(..) {
                                let _ = reply.send(cutoff.clone());
                            }
                        }
                    }
                    if !initialized
                        && sources
                            .iter()
                            .all(|source| broker.source(*source).is_some())
                    {
                        initialized = true;
                        let _ = ready.send(Ok(()));
                    }
                    if !initialized && Instant::now() >= deadline {
                        let _ = ready.send(Err(CaptureFailure::Unavailable));
                        break;
                    }
                }
                background_stop.store(true, Ordering::Release);
                drop(pump_thread);
                let _ = broker.close();
            })
            .map_err(|_| CaptureFailure::Unavailable)?;
        let hub = Arc::new(Self {
            state,
            stop,
            thread: Mutex::new(Some(thread)),
            scheduler,
        });
        started.recv().map_err(|_| CaptureFailure::Unavailable)??;
        Ok(hub)
    }

    /// 冻结订阅起点，各来源基准与游标在来源锁中原子取得。
    pub fn subscribe(
        self: &Arc<Self>,
        delivery: CaptureDelivery,
    ) -> Result<DesktopSubscription, CaptureFailure> {
        let state = self.state.lock().map_err(|_| CaptureFailure::Unavailable)?;
        if state.streams.is_empty() {
            return Err(CaptureFailure::Unavailable);
        }
        let mut cursors = HashMap::new();
        let mut baseline = Vec::new();
        for (source, stream) in &state.streams {
            let (cursor, frame) = stream.subscribe_snapshot(delivery)?;
            cursors.insert(*source, cursor);
            baseline.push((*frame).clone());
        }
        baseline.sort_by_key(|frame| (frame.timing.presented_us, frame.source.0));
        Ok(DesktopSubscription {
            hub: self.clone(),
            cursors,
            baseline,
            incident: state.incident,
            delivery,
        })
    }

    /// 输入仅通知共享采集调度器。
    pub fn wake(&self) {
        self.scheduler.wake();
    }
    /// 应用退出时停止推进并在返回前回收线程；普通订阅释放不调用此方法。
    pub fn shutdown(&self) -> Result<(), CaptureFailure> {
        self.stop.store(true, Ordering::Release);
        self.scheduler.wake();
        let thread = self
            .thread
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?
            .take();
        if let Some(thread) = thread {
            thread.thread().unpark();
            thread.join().map_err(|_| CaptureFailure::Unavailable)?;
        }
        Ok(())
    }
    /// 暂停一次新提交并排空全部已提交 GPU 槽，返回每个来源的确定截止版本。
    pub fn request_barrier(
        &self,
    ) -> Result<
        std::sync::mpsc::Receiver<
            Result<HashMap<CaptureSourceId, argusflow_core::CaptureRevision>, CaptureFailure>,
        >,
        CaptureFailure,
    > {
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        self.state
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?
            .barriers
            .push(reply);
        self.scheduler.wake();
        Ok(receiver)
    }
    /// 获取只暴露等待/唤醒的共享调度契约。
    pub fn scheduler(&self) -> Arc<dyn CaptureScheduler> {
        self.scheduler.clone()
    }
    /// 原生主机累计工作量，可按订阅起止差值记录诊断。
    pub fn counters(&self) -> Result<(u64, u64), CaptureFailure> {
        let state = self.state.lock().map_err(|_| CaptureFailure::Unavailable)?;
        Ok((state.readback_bytes, state.diff_us))
    }
}

impl Drop for DesktopCaptureHub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.scheduler.wake();
        if let Ok(slot) = self.thread.get_mut() {
            if let Some(thread) = slot.take() {
                thread.thread().unpark();
                let _ = thread.join();
            }
        }
    }
}

/// 一组来源的独立订阅；Drop 不关闭共享主机。
pub struct DesktopSubscription {
    hub: Arc<DesktopCaptureHub>,
    cursors: HashMap<CaptureSourceId, CaptureCursor>,
    baseline: Vec<FrameUpdate>,
    incident: u64,
    delivery: CaptureDelivery,
}
impl DesktopSubscription {
    /// 首批完整基准，随后按来源有序取得变化；不等待其它消费者。
    pub fn poll(&mut self) -> Result<Vec<FrameUpdate>, CaptureFailure> {
        let state = self
            .hub
            .state
            .lock()
            .map_err(|_| CaptureFailure::Unavailable)?;
        if state.incident != self.incident {
            self.incident = state.incident;
            if self.delivery == CaptureDelivery::Ordered {
                return Err(state.failure.unwrap_or(CaptureFailure::Unavailable));
            }
            self.cursors.clear();
            self.baseline.clear();
        }
        for (source, stream) in &state.streams {
            if !self.cursors.contains_key(source) {
                let (cursor, frame) = stream.subscribe_snapshot(self.delivery)?;
                self.cursors.insert(*source, cursor);
                self.baseline.push((*frame).clone());
            }
        }
        drop(state);
        let mut frames = std::mem::take(&mut self.baseline);
        for cursor in self.cursors.values_mut() {
            while let Some(delivery) = cursor.next()? {
                let mut frame = (*delivery.frame).clone();
                frame.changes = delivery.changes.into();
                frames.push(frame);
                if self.delivery == CaptureDelivery::Latest {
                    break;
                }
            }
        }
        frames.sort_by_key(|frame| (frame.timing.presented_us, frame.source.0, frame.revision));
        Ok(frames)
    }
}

fn incident(state: &Mutex<HubState>, reason: CaptureFailure) {
    if let Ok(mut state) = state.lock() {
        state.incident = state.incident.saturating_add(1);
        state.failure = Some(reason);
    }
}
