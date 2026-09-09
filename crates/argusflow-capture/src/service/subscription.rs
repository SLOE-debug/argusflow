//! 独立消费游标不会阻塞其他消费者或固定图像。
use super::{CaptureService, ChangeRecord};
use argusflow_capture_contracts::*;
use argusflow_core::FailureKind;

/// 有限日志的独立消费者。
pub struct ChangeSubscription {
    service: CaptureService,
    next: u64,
}
/// 一次订阅交付；缺口元数据没有伪造的日志序号。
pub struct ChangeBatch {
    /// 原始追加日志记录，保留严格递增序号。
    pub records: Vec<ChangeRecord>,
    /// 各来源可能遗漏的历史区间。
    pub gaps: Vec<Gap>,
    /// 已淘汰的全局日志序号闭区间。
    pub lost_sequences: Option<std::ops::RangeInclusive<u64>>,
}
impl CaptureService {
    /// 从此刻开始订阅；需要初始状态时另行调用 sources/pin_at。
    pub fn subscribe_changes(&self) -> ChangeSubscription {
        let next = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sequence
            + 1;
        ChangeSubscription {
            service: self.clone(),
            next,
        }
    }
}
impl ChangeSubscription {
    /// 最多取得 limit 条记录；落后时返回显式缺口并推进到可读起点。
    pub fn poll(&mut self, limit: usize) -> CaptureResult<ChangeBatch> {
        if limit == 0 || limit > 1024 {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "subscription",
                "批量大小必须为 1..=1024",
            ));
        }
        let state = self
            .service
            .inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        let first = state
            .records
            .front()
            .map_or(state.sequence + 1, |record| record.sequence);
        if self.next < first {
            let lost = self.next..=first - 1;
            self.next = first;
            // 全局游标可能漏掉多个来源，每个来源都交付独立缺口。
            return Ok(ChangeBatch {
                records: Vec::new(),
                lost_sequences: Some(lost),
                gaps: state
                    .sources
                    .keys()
                    .map(|source| Gap {
                        source: *source,
                        from: ClockTime(0),
                        through: self.service.now(),
                        reason: GapReason::ConsumerLagged,
                    })
                    .collect(),
            });
        }
        let records: Vec<_> = state
            .records
            .iter()
            .filter(|record| record.sequence >= self.next)
            .take(limit)
            .cloned()
            .collect();
        if let Some(last) = records.last() {
            self.next = last.sequence + 1;
        }
        if records.is_empty() && state.stopped {
            return Err(CaptureError::new(
                FailureKind::Closed,
                "subscription",
                "采样已停止",
            ));
        }
        Ok(ChangeBatch {
            records,
            gaps: Vec::new(),
            lost_sequences: None,
        })
    }
}
