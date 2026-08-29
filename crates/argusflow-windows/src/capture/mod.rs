//! Windows 桌面和窗口画面捕获服务。

use argusflow_core::{AutomationError, BackendKind};

mod device;
mod dpi;
mod error;
mod readback;
mod surface_set;
mod topology;
mod wgc;

pub use topology::{
    CaptureSurfaceMode, WindowRole, WindowTopology, WindowTopologyEntry, WindowTopologyTracker,
};
pub use wgc::WindowsGraphicsCapture;

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
