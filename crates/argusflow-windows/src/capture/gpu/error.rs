//! HRESULT 原始来源及恢复原因，不把错误分类当作无限重试策略。
use argusflow_capture_contracts::{CaptureError, GapReason};
use argusflow_core::{Failure, FailureKind};
use std::error::Error;
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_ACCESS_LOST;
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
pub(in crate::capture) fn recovery_reason(error: &CaptureError) -> GapReason {
    if let CaptureError::Failure(failure) = error
        && let Some(native) = failure
            .source()
            .and_then(|source| source.downcast_ref::<windows::core::Error>())
        && native.code() == DXGI_ERROR_ACCESS_LOST
    {
        return GapReason::TopologyChanged;
    }
    if error.kind() == FailureKind::Timeout {
        GapReason::Unavailable
    } else {
        GapReason::DeviceReset
    }
}
