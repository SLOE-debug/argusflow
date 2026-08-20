//! Windows 桌面和窗口画面捕获服务。

use argusflow_core::{AutomationError, BackendKind};

#[derive(Debug, Default)]
/// 使用 DXGI Desktop Duplication 捕获整个桌面的服务。
pub struct DxgiCapture;

impl DxgiCapture {
    /// 捕获桌面画面；当前因 DXGI 管线未接入而返回后端不可用错误。
    pub fn capture_desktop(&self) -> Result<(), AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::VisualCache,
            message: "DXGI Desktop Duplication 尚未接入".to_owned(),
        })
    }
}

#[derive(Debug, Default)]
/// 使用 Windows.Graphics.Capture 捕获指定窗口的服务。
pub struct WindowsGraphicsCapture;

impl WindowsGraphicsCapture {
    /// 捕获窗口画面；当前因 Windows 图形捕获管线未接入而返回错误。
    pub fn capture_window(&self) -> Result<(), AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::VisualCache,
            message: "Windows.Graphics.Capture 尚未接入".to_owned(),
        })
    }
}
