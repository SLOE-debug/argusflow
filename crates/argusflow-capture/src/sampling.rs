//! 消费者按完整区域取图；先用 GPU 比较令牌，再决定是否读回。
use crate::{CaptureService, observation::summarize};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};

impl RegionSource for CaptureService {
    fn sample(&self, request: SampleRequest, operation: Operation) -> CaptureFuture<RegionSample> {
        let service = self.clone();
        Box::pin(async move {
            let mut cancel = operation.cancel_on_drop();
            let _permit = service
                .inner
                .reads
                .clone()
                .try_acquire_owned()
                .map_err(|_| CaptureError::new(FailureKind::Busy, "sample", "区域采样额度已满"))?;
            if request.quiet.is_zero() || request.quiet > std::time::Duration::from_secs(30) {
                return Err(CaptureError::new(
                    FailureKind::InvalidInput,
                    "sample",
                    "稳定时间超限",
                ));
            }
            let started = service.now();
            loop {
                operation.check("sample")?;
                let ready = {
                    let state = service
                        .inner
                        .state
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    if let Some(error) = &state.failure {
                        return Err(error.clone());
                    }
                    if state.stopped {
                        return Err(CaptureError::new(
                            FailureKind::Closed,
                            "sample",
                            "采样已停止",
                        ));
                    }
                    if let Some(entry) = state.sources.get(&request.source) {
                        if matches!(
                            entry.info.state,
                            SourceState::Unavailable | SourceState::Removed | SourceState::Stopped
                        ) {
                            return Err(CaptureError::new(
                                FailureKind::Unavailable,
                                "sample",
                                "屏幕来源不可用",
                            ));
                        }
                        if !entry.info.bounds.local().contains(request.region) {
                            return Err(CaptureError::new(
                                FailureKind::InvalidInput,
                                "sample",
                                "区域超出屏幕",
                            ));
                        }
                        // 至少确认到本次请求时刻；已健康静止的缓存无需重新等待完整窗口。
                        let from = ClockTime(
                            entry
                                .watermark
                                .0
                                .saturating_sub(request.quiet.as_nanos() as u64),
                        );
                        let summary = summarize(
                            &state,
                            request.source,
                            from,
                            entry.watermark,
                            request.region,
                            &[],
                        )?;
                        let baseline_ready = entry
                            .history
                            .front()
                            .is_some_and(|frame| frame.timing.acquired <= from);
                        let caught_up = entry.history.back().is_some_and(|frame| {
                            frame.timing.presented.unwrap_or(frame.timing.acquired)
                                <= entry.watermark
                        });
                        if caught_up
                            && entry.info.state == SourceState::Ready
                            && entry.watermark >= started
                            && baseline_ready
                            && summary.changes == 0
                            && summary.gaps.is_empty()
                        {
                            entry
                                .history
                                .back()
                                .cloned()
                                .map(|frame| (frame, entry.watermark))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some((snapshot, through)) = ready {
                    let reusable = if let Some(previous) = &request.previous {
                        let old = previous.snapshot.version;
                        let current = snapshot.version;
                        if previous.region == request.region
                            && old.session == current.session
                            && old.source == current.source
                            && old.generation == current.generation
                            && previous.snapshot.pixels.valid()
                        {
                            snapshot
                                .pixels
                                .clone()
                                .compare(
                                    previous.snapshot.pixels.clone(),
                                    vec![request.region],
                                    operation.clone(),
                                )
                                .await?
                                .changed_pixels
                                == 0
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    let content = if reusable {
                        SampleContent::Unchanged
                    } else {
                        SampleContent::Image(
                            snapshot
                                .pixels
                                .clone()
                                .read(request.region, operation.clone())
                                .await?,
                        )
                    };
                    operation.check("sample_complete")?;
                    cancel.disarm();
                    return Ok(RegionSample {
                        observed_version: snapshot.version,
                        observed_through: through,
                        token: ContentToken {
                            snapshot,
                            region: request.region,
                        },
                        content,
                    });
                }
                service.wait_update(&operation).await?;
            }
        })
    }
}
