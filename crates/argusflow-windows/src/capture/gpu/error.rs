//! HRESULT 原始来源及恢复原因，不把错误分类当作无限重试策略。
use argusflow_capture_contracts::CaptureError;
use argusflow_core::{Failure, FailureKind};
pub(in crate::capture) fn failure(error: windows::core::Error) -> CaptureError {
    CaptureError::Failure(
        Failure::new(
            FailureKind::Native,
            "dxgi",
            format!("Windows HRESULT 0x{:08x}", error.code().0 as u32),
        )
        .with_source(error),
    )
}
pub(in crate::capture) fn invalid(message: &str) -> CaptureError {
    CaptureError::new(FailureKind::InvalidInput, "dxgi_geometry", message)
}
