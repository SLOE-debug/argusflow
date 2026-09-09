//! 通过健康水位和历史区间证明交互前版本。
use super::Anchor;
use crate::{CaptureService, ChangeKind};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};

impl CaptureService {
    /// 固定 at 之前的可证明版本。调用方必须使用本服务的 QPC 时钟域。
    pub async fn pin_at(
        &self,
        source: SourceId,
        at: ClockTime,
        options: OperationOptions,
    ) -> CaptureResult<Anchor> {
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        loop {
            operation.check("pin_at")?;
            {
                let state = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(error) = &state.failure {
                    return Err(error.clone());
                }
                if state.stopped {
                    return Err(CaptureError::new(
                        FailureKind::Closed,
                        "pin_at",
                        "采样已停止",
                    ));
                }
                if let Some(entry) = state.sources.get(&source) {
                    if matches!(
                        entry.info.state,
                        SourceState::Unavailable | SourceState::Removed | SourceState::Stopped
                    ) {
                        return Err(CaptureError::new(
                            FailureKind::Unavailable,
                            "pin_at",
                            "来源不可用",
                        ));
                    }
                    if entry.watermark >= at {
                        if at <= entry.log_floor {
                            return Err(gap(source, at, GapReason::HistoryEvicted));
                        }
                        for record in &state.records {
                            if let ChangeKind::Gap(known) = &record.kind
                                && known.source == source
                                && known.from <= at
                                && at <= known.through
                            {
                                return Err(CaptureError::HistoryGap(known.clone()));
                            }
                        }
                        // 不同线程 ±1 QPC tick 的顺序不确定，不能选作确定的交互前版本。
                        let uncertainty = 1_000_000_000_u64.div_ceil(self.clock().frequency.max(1));
                        let snapshot = entry.history.iter().rev().find(|snapshot| {
                            snapshot
                                .timing
                                .presented
                                .unwrap_or(snapshot.timing.acquired)
                                .0
                                .saturating_add(uncertainty)
                                < at.0
                        });
                        let Some(snapshot) = snapshot else {
                            return Err(gap(source, at, GapReason::HistoryEvicted));
                        };
                        if entry.history.iter().any(|frame| {
                            frame
                                .timing
                                .presented
                                .is_some_and(|present| present.0.abs_diff(at.0) <= uncertainty)
                        }) {
                            return Err(gap(source, at, GapReason::AmbiguousTime));
                        }
                        if !snapshot.pixels.valid() {
                            return Err(gap(source, at, GapReason::DeviceReset));
                        }
                        return Ok(Anchor {
                            snapshot: snapshot.clone(),
                            at,
                            expires: self.now().after(self.inner.config.anchor_lifetime),
                        });
                    }
                }
            }
            self.wait_update(&operation).await?;
        }
    }
}
fn gap(source: SourceId, at: ClockTime, reason: GapReason) -> CaptureError {
    CaptureError::HistoryGap(Gap {
        source,
        from: at,
        through: at,
        reason,
    })
}
