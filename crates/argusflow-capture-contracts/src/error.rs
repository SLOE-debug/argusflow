//! 采集与区域处理错误，保留阶段、资源及原始来源。
use argusflow_core::{Failure, FailureKind};
/// 采样操作结果。
pub type CaptureResult<T> = Result<T, CaptureError>;
/// 平台无关的采集失败。
#[derive(Debug, Clone, thiserror::Error)]
pub enum CaptureError {
    /// 通用操作失败，保留阶段、资源与原始来源。
    #[error(transparent)]
    Failure(#[from] Failure),
}
impl CaptureError {
    /// 构造不包含用户图像的操作失败。
    pub fn new(kind: FailureKind, stage: &'static str, message: impl Into<String>) -> Self {
        Self::Failure(Failure::new(kind, stage, message))
    }
    /// 返回统一错误分类。
    pub fn kind(&self) -> FailureKind {
        match self {
            Self::Failure(error) => error.kind(),
        }
    }
}
