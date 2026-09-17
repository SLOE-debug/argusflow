//! 按请求检查完整帧历史；只比较目标区域，不启动额外 GPU 差分后台。
use crate::{pixels, validity::Validity};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// 多消费者共享的区域采样适配器；原生帧源的启动和关闭由装配层负责。
#[derive(Clone)]
pub struct FrameSampler {
    source: Arc<dyn DesktopFrameSource>,
    budget: ByteBudget,
    reads: Arc<Semaphore>,
}
impl FrameSampler {
    /// 区域图像最多占用 128 MiB，并发请求最多 8 个；克隆共享预算。
    pub fn new(source: Arc<dyn DesktopFrameSource>) -> CaptureResult<Self> {
        Ok(Self {
            source,
            budget: ByteBudget::new(128 * 1024 * 1024)?,
            reads: Arc::new(Semaphore::new(8)),
        })
    }
    /// 当前来源快照，不触发截图。
    pub fn sources(&self) -> CaptureResult<Vec<SourceInfo>> {
        Ok(self
            .source
            .history()?
            .into_iter()
            .map(|history| history.source)
            .collect())
    }
}
impl RegionSource for FrameSampler {
    fn sample(&self, request: SampleRequest, operation: Operation) -> CaptureFuture<RegionSample> {
        let sampler = self.clone();
        Box::pin(async move {
            let mut cancel = operation.cancel_on_drop();
            let _permit = sampler
                .reads
                .clone()
                .try_acquire_owned()
                .map_err(|_| failure(FailureKind::Busy, "区域采样并发已满"))?;
            if request.quiet > Duration::from_secs(1) {
                return Err(failure(
                    FailureKind::InvalidInput,
                    "稳定观察窗口不能超过一秒",
                ));
            }
            let started = sampler.source.now();
            loop {
                operation.check("frame_sample")?;
                let histories = sampler.source.history()?;
                let history = histories
                    .iter()
                    .find(|history| history.source.id == request.source)
                    .ok_or_else(|| failure(FailureKind::Unavailable, "屏幕来源不存在"))?;
                if let Some(error) = &history.source.failure {
                    return Err(error.clone());
                }
                if matches!(
                    history.source.state,
                    SourceState::Unavailable | SourceState::Removed | SourceState::Stopped
                ) {
                    return Err(failure(FailureKind::Unavailable, "屏幕来源不可用"));
                }
                if !history.source.bounds.local().contains(request.region) {
                    return Err(failure(FailureKind::InvalidInput, "区域超出屏幕"));
                }
                if history.source.state == SourceState::Ready
                    && history.checked >= started
                    && let Some(frame) = stable(history, request.region, request.quiet, &operation)?
                {
                    let image =
                        pixels::crop(&frame.image, request.region, &sampler.budget, &operation)?;
                    let local = PixelRect::new(0, 0, image.width(), image.height())?;
                    let reusable = if let Some(previous) = &request.previous {
                        let old = previous.snapshot.version;
                        old.session == frame.version.session
                            && old.source == frame.version.source
                            && old.generation == frame.version.generation
                            && previous.snapshot.bounds == frame.source.bounds
                            && previous.region == request.region
                            && previous.snapshot.validity.valid()
                            && !pixels::differs(&previous.image, &image, local, &operation)?
                    } else {
                        false
                    };
                    let snapshot = Arc::new(Snapshot {
                        version: frame.version,
                        bounds: frame.source.bounds,
                        timing: frame.timing,
                        validity: Arc::new(Validity {
                            source: sampler.source.clone(),
                            version: frame.version,
                            bounds: frame.source.bounds,
                        }),
                    });
                    if !snapshot.validity.valid() {
                        return Err(failure(FailureKind::StaleHandle, "采样过程中来源已失效"));
                    }
                    operation.check("frame_sample_complete")?;
                    cancel.disarm();
                    return Ok(RegionSample {
                        token: ContentToken {
                            snapshot,
                            region: request.region,
                            image: image.clone(),
                        },
                        content: if reusable {
                            SampleContent::Unchanged
                        } else {
                            SampleContent::Image(image)
                        },
                        observed_version: frame.version,
                        observed_through: history.checked,
                    });
                }
                tokio::time::sleep(operation.remaining().min(Duration::from_millis(10))).await;
            }
        })
    }
}
fn failure(kind: FailureKind, message: &str) -> CaptureError {
    CaptureError::new(kind, "frame_sample", message)
}

fn stable<'a>(
    history: &'a FrameHistory,
    region: PixelRect,
    quiet: Duration,
    operation: &Operation,
) -> CaptureResult<Option<&'a DesktopFrame>> {
    let Some(latest) = history.frames.last() else {
        return Ok(None);
    };
    let from = ClockTime(history.checked.0.saturating_sub(quiet.as_nanos() as u64));
    let Some(start) = history
        .frames
        .iter()
        .rposition(|frame| frame.timing.frozen <= from)
    else {
        return Ok(None);
    };
    for frame in &history.frames[start..] {
        if frame.version.source != history.source.id
            || frame.version.generation != history.source.generation
            || frame.source.bounds != history.source.bounds
            || frame.version.session != latest.version.session
        {
            return Err(failure(FailureKind::StaleHandle, "帧历史跨越来源重建"));
        }
        if frame.image.width() != history.source.bounds.width()
            || frame.image.height() != history.source.bounds.height()
        {
            return Err(failure(
                FailureKind::Unsupported,
                "OCR 采样要求原始分辨率帧，不能使用缩略图",
            ));
        }
        if frame.timing.frozen > history.checked {
            return Ok(None);
        }
        if pixels::differs(&latest.image, &frame.image, region, operation)? {
            return Ok(None);
        }
    }
    Ok(Some(latest))
}
