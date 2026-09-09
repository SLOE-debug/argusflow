//! 录制消费者：整场持续接收变化，不等待输入、OCR 或画面稳定。

use crate::{
    EvidenceCollector, RecorderError,
    screen_archive::*,
    screen_archive_writer::{ArchiveResult, ScreenArchiveWriter},
};
use argusflow_capture::PixelRect;
use argusflow_core::{CaptureFailure, CaptureGeneration, CaptureSourceId, InspectionRect};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct Checkpoint {
    last: ScreenFrameId,
    generation: CaptureGeneration,
    count: u32,
    timestamp_us: u64,
}

/// 生命周期拥有一个变化消费者，退出时排空已接受的编码任务。
pub(crate) struct ScreenRecording {
    collector: Arc<EvidenceCollector>,
    origin_us: u64,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    result: ArchiveResult,
    wake: Arc<crate::sampling_wait::SamplingWait>,
    capture_wake: Arc<dyn argusflow_capture::CaptureScheduler>,
}

impl ScreenRecording {
    pub(crate) fn start(
        collector: Arc<EvidenceCollector>,
        directory: PathBuf,
        origin_us: u64,
    ) -> Result<Self, RecorderError> {
        let writer = ScreenArchiveWriter::start(directory)?;
        let wake = crate::sampling_wait::SamplingWait::new()?;
        let hub = collector
            .screen_hub(crate::sampling_wait::SamplingWait::new()?)
            .map_err(|reason| RecorderError::CaptureStartup {
                stage: crate::CaptureStartupStage::DesktopBaseline,
                reason,
            })?;
        let capture_wake = hub.scheduler();
        let initial_counters = hub
            .counters()
            .map_err(|reason| RecorderError::CaptureStartup {
                stage: crate::CaptureStartupStage::CaptureCounters,
                reason,
            })?;
        let mut subscription = hub
            .subscribe(argusflow_core::CaptureDelivery::Ordered)
            .map_err(|reason| RecorderError::CaptureStartup {
                stage: crate::CaptureStartupStage::OrderedSubscription,
                reason,
            })?;
        let background_wake = wake.clone();
        let result = writer.result();
        let stop = Arc::new(AtomicBool::new(false));
        let background_stop = stop.clone();
        let background_result = result.clone();
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("argusflow-screen-stream".into())
            .spawn(move || {
                let mut checkpoints = HashMap::<CaptureSourceId, Checkpoint>::new();
                let mut sequence = 0;
                let deadline = Instant::now() + Duration::from_secs(3);
                let mut baseline_ready = false;
                let mut drain_started = None;
                let mut barrier = None;
                while !background_result.failed() {
                    let stopping = background_stop.load(Ordering::Acquire);
                    if stopping && drain_started.is_none() {
                        drain_started = Some(Instant::now());
                        match hub.request_barrier() {
                            Ok(receiver) => barrier = Some(receiver),
                            Err(reason) => {
                                background_result.fail(reason);
                                break;
                            }
                        }
                    }
                    if drain_started
                        .is_some_and(|started| started.elapsed() > Duration::from_millis(250))
                    {
                        background_result.fail(CaptureFailure::Unavailable);
                        break;
                    }
                    let cutoff = match barrier.as_ref().map(|receiver| receiver.try_recv()) {
                        Some(Ok(Ok(cutoff))) => Some(cutoff),
                        Some(Ok(Err(reason))) => {
                            background_result.fail(reason);
                            break;
                        }
                        Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                            background_result.fail(CaptureFailure::Unavailable);
                            break;
                        }
                        _ => None,
                    };
                    let updates = match subscription.poll() {
                        Ok(updates) => updates,
                        Err(reason) => {
                            background_result.fail(reason);
                            break;
                        }
                    };
                    for frame in updates {
                        if cutoff.as_ref().is_some_and(|cutoff| {
                            cutoff
                                .get(&frame.source)
                                .is_none_or(|revision| frame.revision > *revision)
                        }) {
                            continue;
                        }
                        sequence += 1;
                        let id = ScreenFrameId(sequence);
                        let previous = checkpoints.get(&frame.source);
                        let checkpoint = previous.is_none_or(|old| {
                            old.generation != frame.generation
                                || old.count >= 120
                                || frame.timing.presented_us.saturating_sub(old.timestamp_us)
                                    >= 2_000_000
                        });
                        let bounds = frame.snapshot.bounds();
                        let regions = if checkpoint {
                            vec![PixelRect {
                                x: 0,
                                y: 0,
                                width: bounds.width as u32,
                                height: bounds.height as u32,
                            }]
                        } else {
                            frame.changes.to_vec()
                        };
                        let record = ScreenFrame {
                            id,
                            source: frame.source,
                            generation: frame.generation,
                            revision: frame.revision,
                            presented_us: frame.timing.presented_us.saturating_sub(origin_us),
                            frozen_us: frame.timing.frozen_us.saturating_sub(origin_us),
                            bounds,
                            previous: if checkpoint {
                                None
                            } else {
                                previous.map(|old| old.last)
                            },
                            changes: frame
                                .changes
                                .iter()
                                .map(|rect| InspectionRect {
                                    x: bounds.x + f64::from(rect.x),
                                    y: bounds.y + f64::from(rect.y),
                                    width: rect.width.into(),
                                    height: rect.height.into(),
                                })
                                .collect(),
                            patches: Vec::new(),
                        };
                        checkpoints.insert(
                            frame.source,
                            Checkpoint {
                                last: id,
                                generation: frame.generation,
                                count: if checkpoint {
                                    0
                                } else {
                                    previous.map_or(0, |old| old.count + 1)
                                },
                                timestamp_us: if checkpoint {
                                    frame.timing.presented_us
                                } else {
                                    previous
                                        .map_or(frame.timing.presented_us, |old| old.timestamp_us)
                                },
                            },
                        );
                        if let Err(reason) = writer.submit(frame, record, regions) {
                            background_result.fail(reason);
                            break;
                        }
                    }
                    if cutoff.is_some() {
                        break;
                    }
                    if !baseline_ready
                        && checkpoints.keys().all(|source| {
                            background_result
                                .snapshot()
                                .frames
                                .iter()
                                .any(|frame| frame.source == *source)
                        })
                    {
                        baseline_ready = true;
                        let _ = ready.send(Ok(()));
                    }
                    if !baseline_ready && Instant::now() >= deadline {
                        background_result.fail(CaptureFailure::Unavailable);
                        break;
                    }
                    if background_wake.wait().is_err() {
                        background_result.fail(CaptureFailure::Unavailable);
                        break;
                    }
                }
                if !baseline_ready {
                    let reason = match background_result.snapshot().completeness {
                        ScreenCompleteness::Incomplete { reason } => reason,
                        _ => CaptureFailure::Unavailable,
                    };
                    let _ = ready.send(Err(RecorderError::CaptureStartup {
                        stage: crate::CaptureStartupStage::ArchiveBaseline,
                        reason,
                    }));
                }
                drop(writer);
                if let Ok((bytes, diff_us)) = hub.counters() {
                    background_result.observe_capture(
                        bytes.saturating_sub(initial_counters.0) as usize,
                        diff_us.saturating_sub(initial_counters.1),
                    );
                }
            })
            .map_err(|_| RecorderError::WorkerUnavailable)?;
        match started.recv() {
            Ok(Ok(())) => Ok(Self {
                collector,
                origin_us,
                stop,
                thread: Some(thread),
                result,
                wake,
                capture_wake,
            }),
            failed => {
                stop.store(true, Ordering::Release);
                let _ = thread.join();
                Err(match failed {
                    Ok(Err(error)) => error,
                    _ => RecorderError::WorkerUnavailable,
                })
            }
        }
    }

    pub(crate) fn result(&self) -> ArchiveResult {
        self.result.clone()
    }
    pub(crate) fn wake(&self) -> Arc<dyn argusflow_capture::CaptureScheduler> {
        self.capture_wake.clone()
    }

    pub(crate) fn finish(&mut self) -> ScreenTimeline {
        self.stop.store(true, Ordering::Release);
        self.wake.wake();
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
        let mut timeline = self.result.snapshot();
        if timeline.completeness == ScreenCompleteness::Complete {
            if let Ok(now) = self.collector.screen_clock_us() {
                timeline.duration_us = now.saturating_sub(self.origin_us);
            }
        }
        timeline
    }
}

impl Drop for ScreenRecording {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
#[path = "screen_recording_tests.rs"]
mod tests;
