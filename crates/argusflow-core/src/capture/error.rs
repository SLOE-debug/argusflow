//! 共享采集错误，不包含 OCR、工作流或平台实现细节。

use super::frame::{FrameId, PhysicalRect};
use crate::WindowIdentity;
use thiserror::Error;

/// 帧源、像素和订阅不变量的失败分类。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CaptureError {
    /// 原生来源不可用。
    #[error("capture unavailable: {message}")]
    CaptureUnavailable { message: String },
    /// 来源身份已改变，旧帧不得用于新窗口。
    #[error("window identity changed from {expected:?} to {actual:?}")]
    WindowIdentityChanged {
        expected: WindowIdentity,
        actual: Option<WindowIdentity>,
    },
    /// 指定预算内没有新帧。
    #[error("frame timed out after {timeout_ms}ms")]
    FrameTimeout { timeout_ms: u64 },
    /// 像素布局不满足边界约束。
    #[error("invalid frame: {message}")]
    InvalidFrame { message: String },
    /// 区域超出帧范围。
    #[error("invalid ROI {rect:?} for frame {frame_id:?}")]
    InvalidRoi {
        rect: PhysicalRect,
        frame_id: FrameId,
    },
}
