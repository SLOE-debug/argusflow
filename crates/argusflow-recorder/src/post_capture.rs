//! 独立屏幕观察线程：一次采样和 diff 服务全部事件，摄入线程只关联历史帧。
use crate::{
    EvidenceCollector, PhysicalEvent, PhysicalInput, RecorderError,
    screen_observer::{ScreenHistory, ScreenSample},
    screenshot_pipeline::{PendingScreenshot, ScreenshotWriter},
};
use argusflow_core::{InspectionFailure, InspectionProbe, ScreenPoint, WindowIdentity};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

/// 每个事件只保存状态，不保存或重复比较整帧像素。
struct CaptureJob {
    /// 事件相关窗口内的变化包围盒，不使用全桌面变化直接裁剪。
    changed_bounds: Option<argusflow_core::InspectionRect>,
    scope: Option<argusflow_core::InspectionRect>,
    local_revision: u64,
    /// 范围相同但窗口身份改变也要重新累计变化和安静区间。
    result_window: Option<WindowIdentity>,
    event: PhysicalEvent,
    elapsed_ms: u64,
    source_window: Option<WindowIdentity>,
    settle: crate::capture_settle::CaptureSettle,
    result: oneshot::Sender<Result<crate::ScreenshotEvidence, InspectionFailure>>,
}

/// 会话拥有一个画面线程与一个 PNG 线程；历史容量由 ScreenHistory 限制。
pub(crate) struct PostCapture {
    writer: Arc<ScreenshotWriter>,
    history: Arc<Mutex<ScreenHistory>>,
    sender: Option<SyncSender<CaptureJob>>,
    thread: Option<JoinHandle<()>>,
}

impl PostCapture {
    pub(crate) fn start(
        collector: Arc<EvidenceCollector>,
        writer: ScreenshotWriter,
        origin: Instant,
        enabled: bool,
    ) -> Result<Self, RecorderError> {
        let (sender, receiver) = mpsc::sync_channel::<CaptureJob>(16);
        let writer = Arc::new(writer);
        let history = Arc::new(Mutex::new(ScreenHistory::default()));
        let background_writer = writer.clone();
        let background_history = history.clone();
        let thread = std::thread::Builder::new()
            .name("argusflow-screen-observer".into())
            .spawn(move || {
                let mut pending = Vec::<CaptureJob>::new();
                let mut closed = false;
                while !closed || !pending.is_empty() {
                    let cycle = Instant::now();
                    loop {
                        match receiver.try_recv() {
                            Ok(job) if pending.len() < 16 => pending.push(job),
                            Ok(job) => {
                                let _ = job.result.send(Err(InspectionFailure::Unavailable));
                            }
                            Err(mpsc::TryRecvError::Disconnected) => {
                                closed = true;
                                break;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                        }
                    }
                    if closed && pending.is_empty() {
                        break;
                    }
                    // 没有输入时也观察；隐私禁止截图时完全不调用像素采集。
                    let sample = if enabled {
                        collector.capture_desktop().ok().and_then(|frame| {
                            let captured_ms = origin.elapsed().as_millis() as u64;
                            let regions = frame
                                .as_ref()
                                .map(|frame| {
                                    let previous = background_history
                                        .lock()
                                        .ok()
                                        .and_then(|history| history.latest_frame());
                                    crate::evidence_region::changes(previous.as_deref(), frame)
                                })
                                .unwrap_or_default();
                            let changed = !regions.is_empty();
                            background_history
                                .lock()
                                .ok()
                                .and_then(|mut history| match frame {
                                    Some(frame) => Some(history.push_changed(
                                        frame,
                                        captured_ms,
                                        cycle.elapsed().as_millis() as u64,
                                        changed,
                                        regions,
                                    )),
                                    None => history.unchanged(captured_ms),
                                })
                        })
                    } else {
                        None
                    };
                    if sample.is_none() {
                        pending.iter_mut().for_each(|job| job.settle.invalidate());
                        if let Ok(mut history) = background_history.lock() {
                            history.clear();
                        }
                    }
                    let mut index = 0;
                    while index < pending.len() {
                        let now_ms = origin.elapsed().as_millis() as u64;
                        let job = &mut pending[index];
                        // 原窗口 show 只关联该窗口；按键结果跟随前台，点击结果按点查询。
                        let current = match job.event.input {
                            PhysicalInput::Window { window, .. } => {
                                collector.window_context(window)
                            }
                            PhysicalInput::Mouse { point, .. }
                            | PhysicalInput::Wheel { point, .. } => {
                                collector.context(InspectionProbe::Point(point))
                            }
                            _ => collector.context(InspectionProbe::Focus),
                        }
                        .ok();
                        let scope = current.as_ref().map(|context| context.bounds);
                        let result_window = current.as_ref().map(|context| context.window);
                        if scope != job.scope || result_window != job.result_window {
                            job.scope = scope;
                            job.result_window = result_window;
                            job.changed_bounds = None;
                            job.settle = crate::capture_settle::CaptureSettle::new(
                                job.event.input,
                                Some(job.local_revision),
                            );
                        }
                        let age = now_ms.saturating_sub(job.elapsed_ms);
                        let stable = sample.as_ref().is_some_and(|sample| observe(job, sample));
                        if !stable && age < 1200 {
                            index += 1;
                            continue;
                        }
                        let job = pending.swap_remove(index);
                        if let Some(sample) = sample.as_ref().filter(|_| age <= 1500) {
                            let pointer = match job.event.input {
                                PhysicalInput::Mouse { point, .. }
                                | PhysicalInput::Wheel { point, .. } => Some(point),
                                _ => None,
                            };
                            let same_target = pointer
                                .and_then(|point| {
                                    collector.context(InspectionProbe::Point(point)).ok()
                                })
                                .is_some_and(|context| Some(context.window) == job.source_window);
                            let mark_click = same_target
                                && matches!(
                                    job.event.input,
                                    PhysicalInput::Mouse {
                                        button: crate::MouseButton::Left,
                                        phase: crate::InputPhase::Down,
                                        ..
                                    }
                                );
                            background_writer.submit_post(
                                job.event.sequence,
                                match job.scope.ok_or(InspectionFailure::ContextChanged).and_then(
                                    |scope| {
                                        crate::evidence_region::crop(
                                            &sample.frame,
                                            scope,
                                            job.changed_bounds,
                                            pointer,
                                        )
                                    },
                                ) {
                                    Ok(frame) => frame,
                                    Err(reason) => {
                                        let _ = job.result.send(Err(reason));
                                        continue;
                                    }
                                },
                                sample.observed_ms,
                                sample.duration_ms,
                                pointer,
                                mark_click,
                                stable,
                                job.result,
                            );
                        } else {
                            let _ = job.result.send(Err(InspectionFailure::Timeout));
                        }
                    }
                    // 约 30Hz 上限；采样较慢时自然降频，不累计补跑任务。
                    if let Some(wait) = Duration::from_millis(33).checked_sub(cycle.elapsed()) {
                        std::thread::sleep(wait);
                    }
                }
            })
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        Ok(Self {
            writer,
            history,
            sender: Some(sender),
            thread: Some(thread),
        })
    }

    /// 只选择实际输入时刻之前的屏幕帧；缺失时明确失败，禁止同步补拍。
    pub(crate) fn target(
        &self,
        sequence: u64,
        event_ms: u64,
        point: ScreenPoint,
        scope: Option<argusflow_core::InspectionRect>,
    ) -> Result<PendingScreenshot, InspectionFailure> {
        let sample = self
            .history
            .try_lock()
            .map_err(|_| InspectionFailure::Unavailable)?
            .before(event_ms)?;
        Ok(self.writer.submit_target(
            sequence,
            crate::evidence_region::crop(
                &sample.frame,
                scope.unwrap_or(argusflow_core::InspectionRect {
                    x: f64::from(point.x) - 192.0,
                    y: f64::from(point.y) - 128.0,
                    width: 384.0,
                    height: 256.0,
                }),
                None,
                Some(point),
            )?,
            sample.observed_ms,
            sample.duration_ms,
            point,
        ))
    }

    pub(crate) fn submit(
        &self,
        event: PhysicalEvent,
        elapsed_ms: u64,
        source_window: Option<WindowIdentity>,
    ) -> Result<PendingScreenshot, InspectionFailure> {
        let (result, receiver) = oneshot::channel();
        let revision = self
            .history
            .try_lock()
            .ok()
            .and_then(|history| history.before(elapsed_ms).ok())
            .map(|sample| sample.revision);
        self.sender
            .as_ref()
            .ok_or(InspectionFailure::Unavailable)?
            .try_send(CaptureJob {
                changed_bounds: None,
                scope: None,
                local_revision: 0,
                result_window: None,
                event,
                elapsed_ms,
                source_window,
                settle: crate::capture_settle::CaptureSettle::new(event.input, revision.map(|_| 0)),
                result,
            })
            .map_err(|_| InspectionFailure::Unavailable)?;
        Ok(receiver)
    }
}

/// 画面变化由共享 diff 代数提供；事件不再调用截图或逐帧像素比较。
fn observe(job: &mut CaptureJob, sample: &ScreenSample) -> bool {
    if sample.observed_ms < job.elapsed_ms {
        return false;
    }
    let mut changed = false;
    if let Some(scope) = job.scope {
        for rect in sample
            .changes
            .iter()
            .filter_map(|rect| crate::evidence_region::intersect(*rect, scope))
        {
            crate::evidence_region::include(&mut job.changed_bounds, rect);
            changed = true;
        }
    }
    if changed {
        job.local_revision += 1;
    }
    job.settle
        .observe(sample.observed_ms - job.elapsed_ms, job.local_revision)
}

impl Drop for PostCapture {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
