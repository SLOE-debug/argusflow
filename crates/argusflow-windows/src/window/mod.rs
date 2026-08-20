//! Win32 窗口枚举与管理服务。

use argusflow_core::{AutomationError, BackendKind};

#[derive(Debug, Default)]
/// 提供 Win32 窗口枚举与查询能力的服务。
pub struct WindowService;

impl WindowService {
    /// 枚举桌面窗口；当前因 Win32 窗口服务未接入而返回错误。
    pub fn enumerate(&self) -> Result<(), AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::WindowsUia,
            message: "Win32 window service 尚未接入".to_owned(),
        })
    }
}
