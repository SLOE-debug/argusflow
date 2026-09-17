//! 装配层通用的尝试记录与有界调用参数。
use argusflow_recorder::{Outcome, RecordData, Stage};

pub(crate) fn attempt(raw: &[u64], stage: Stage, outcome: Outcome, reason: &str) -> RecordData {
    RecordData::Attempt {
        raw: raw.to_vec(),
        stage,
        outcome,
        reason: reason.into(),
    }
}
pub(crate) fn options(ms: u64) -> argusflow_core::OperationOptions {
    // 调用方只传非零且不超过24小时的内部常量。
    argusflow_core::OperationOptions::new(std::time::Duration::from_millis(ms))
        .expect("内部证据时限必须有效")
}
