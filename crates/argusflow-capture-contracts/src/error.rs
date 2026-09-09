//! 采样错误和不连续区间；缺口不能转换为正常图像。
use crate::{ClockTime, SourceId};
use argusflow_core::{Failure, FailureKind};

/// 采样操作结果。
pub type CaptureResult<T> = Result<T, CaptureError>;

/// 无法完整观察的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapReason {
    /// 系统合并了多个呈现。
    Accumulated,
    /// 未固定历史被预算或保留期淘汰。
    HistoryEvicted,
    /// 消费者落后于元数据日志。
    ConsumerLagged,
    /// 图形设备重建。
    DeviceReset,
    /// 显示器或桌面拓扑改变。
    TopologyChanged,
    /// 有界队列或资源预算耗尽。
    Capacity,
    /// 来源不可用或 GPU 超时。
    Unavailable,
    /// 内容被操作系统保护。
    Protected,
    /// 时钟顺序不能被证明。
    AmbiguousTime,
}

/// 闭区间内的历史不确定性，时间属于来源的时钟域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// 涉及的屏幕来源。
    pub source: SourceId,
    /// 区间下界。
    pub from: ClockTime,
    /// 区间上界。
    pub through: ClockTime,
    /// 明确原因。
    pub reason: GapReason,
}

/// 携带原始失败或明确历史缺口的错误。
#[derive(Debug, Clone, thiserror::Error)]
pub enum CaptureError {
    /// 通用操作失败，保留阶段、资源与原始来源。
    #[error(transparent)]
    Failure(#[from] Failure),
    /// 请求所依赖的历史不完整。
    #[error("capture history gap: {0:?}")]
    HistoryGap(Gap),
}
impl CaptureError {
    /// 构造不包含用户图像的操作失败。
    pub fn new(kind: FailureKind, stage: &'static str, message: impl Into<String>) -> Self {
        Self::Failure(Failure::new(kind, stage, message))
    }
    /// 返回统一的错误分类；历史缺口归为失效引用。
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Failure(error) => error.kind(),
            Self::HistoryGap(_) => FailureKind::StaleHandle,
        }
    }
}
