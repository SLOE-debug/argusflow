//! 稳定等待始终核对处理水位，原生卡住不能通过计时器成为稳定。
use super::{Anchor, Observation, ObservationStatus, ProcessSummary, summarize};
use crate::{CaptureService, ObservationOptions, reading_regions, subtract_regions};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};

impl CaptureService {
    /// 观察一个来源的区域；多个锚点可以独立得到相同终点版本。
    pub async fn observe(
        &self,
        anchor: &Anchor,
        scope: PixelRect,
        policy: ObservationOptions,
        options: OperationOptions,
    ) -> CaptureResult<Observation> {
        policy.validate()?;
        if self.now() > anchor.expires || !anchor.snapshot.pixels.valid() {
            return Err(CaptureError::new(
                FailureKind::StaleHandle,
                "observe",
                "锚点已失效",
            ));
        }
        if !anchor.snapshot.bounds.local().contains(scope)
            || anchor.version().session != self.clock().session
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "observe",
                "观察区域或会话不匹配",
            ));
        }
        let _permit = self
            .inner
            .observations
            .clone()
            .try_acquire_owned()
            .map_err(|_| CaptureError::new(FailureKind::Busy, "observe", "并发观察已满"))?;
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let stop_at = self.now().after(policy.maximum);
        let started = self.now();
        let wait_deadline = std::time::Instant::now() + policy.maximum;
        let mut result = Observation {
            status: ObservationStatus::TimedOutUnstable,
            before: anchor.version(),
            after: None,
            through: anchor.at,
            process: ProcessSummary::default(),
            exact_regions: Vec::new(),
            images: Vec::new(),
        };
        loop {
            if operation.is_cancelled() {
                result.status = ObservationStatus::Cancelled;
                return Ok(result);
            }
            if operation.remaining().is_zero()
                || self.now() >= stop_at
                || std::time::Instant::now() >= wait_deadline
            {
                return Ok(result);
            }
            // 已接受的观察固定起点，普通租约到期不撤销正在执行的请求。
            if !anchor.snapshot.pixels.valid() {
                result.status = ObservationStatus::HistoryGap;
                return Ok(result);
            }
            let stable = {
                let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(error) = &state.failure {
                    return Err(error.clone());
                }
                let Some(entry) = state.sources.get(&anchor.version().source) else {
                    result.status = ObservationStatus::SourceUnavailable;
                    return Ok(result);
                };
                if state.stopped
                    || entry.info.state != SourceState::Ready
                    || entry.info.generation != anchor.version().generation
                {
                    result.status = ObservationStatus::SourceUnavailable;
                    return Ok(result);
                }
                result.through = entry.watermark;
                result.process = summarize(
                    &state,
                    entry.info.id,
                    anchor.at,
                    entry.watermark,
                    scope,
                    &policy.ignored,
                )?;
                if !result.process.gaps.is_empty() {
                    result.status = ObservationStatus::HistoryGap;
                    return Ok(result);
                }
                let caught_up = entry.history.back().is_some_and(|frame| {
                    frame.timing.presented.unwrap_or(frame.timing.acquired) <= entry.watermark
                });
                if caught_up
                    && entry.watermark >= started
                    && entry.watermark.elapsed_since(anchor.at) >= policy.minimum
                    && entry.watermark.elapsed_since(result.process.last_change) >= policy.quiet
                {
                    entry.history.back().cloned()
                } else {
                    None
                }
            };
            if let Some(after) = stable {
                let candidates = subtract_regions(&result.process.regions, &policy.ignored)?;
                let changes = after
                    .pixels
                    .clone()
                    .compare(
                        anchor.snapshot.pixels.clone(),
                        candidates,
                        operation.clone(),
                    )
                    .await?;
                result.after = Some(after.version);
                result.exact_regions = changes.regions;
                let regions = subtract_regions(
                    &reading_regions(
                        &result.exact_regions,
                        policy.padding,
                        anchor.snapshot.bounds.local(),
                    )?,
                    &policy.ignored,
                )?;
                for region in regions {
                    result.images.push((
                        region,
                        after.pixels.clone().read(region, operation.clone()).await?,
                    ));
                }
                operation.check("observe_complete")?;
                result.status = if changes.changed_pixels == 0 {
                    ObservationStatus::StableUnchanged
                } else {
                    ObservationStatus::StableChanged
                };
                return Ok(result);
            }
            match self.wait_update(&operation).await {
                Ok(()) => {}
                Err(error) if error.kind() == FailureKind::Timeout => return Ok(result),
                Err(error) => return Err(error),
            }
        }
    }
    /// 从仍有效的锚点按需读取图像；不会获取较新版本替代它。
    pub async fn read_regions(
        &self,
        anchor: &Anchor,
        regions: &[PixelRect],
        options: OperationOptions,
    ) -> CaptureResult<Vec<(PixelRect, PixelImage)>> {
        if self.now() > anchor.expires
            || anchor.version().session != self.clock().session
            || !anchor.snapshot.pixels.valid()
        {
            return Err(CaptureError::new(
                FailureKind::StaleHandle,
                "read_regions",
                "锚点已失效",
            ));
        }
        let _permit =
            self.inner.reads.clone().try_acquire_owned().map_err(|_| {
                CaptureError::new(FailureKind::Busy, "read_regions", "读取额度已满")
            })?;
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let mut images = Vec::new();
        for region in crate::normalize_regions(regions)? {
            images.push((
                region,
                anchor
                    .snapshot
                    .pixels
                    .clone()
                    .read(region, operation.clone())
                    .await?,
            ));
        }
        Ok(images)
    }
}
