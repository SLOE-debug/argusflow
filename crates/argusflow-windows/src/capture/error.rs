//! Windows 捕获 API 错误到视觉领域错误的边界映射。

use std::fmt::Display;

use argusflow_vision::VisionError;

/// 把底层 Windows 错误包装成不泄漏像素内容的捕获错误。
pub(super) fn capture_error(context: &str, error: impl Display) -> VisionError {
    VisionError::CaptureUnavailable {
        message: format!("{context}: {error}"),
    }
}

/// 创建一个不依赖底层 COM 错误的参数错误。
pub(super) fn invalid_capture(message: impl Into<String>) -> VisionError {
    VisionError::CaptureUnavailable {
        message: message.into(),
    }
}
