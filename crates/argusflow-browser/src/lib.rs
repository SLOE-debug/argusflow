//! 浏览器自动化后端及其 Chrome DevTools Protocol 接入点。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

use argusflow_agent::ActionBackend;
use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind, Selector};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 基于 Chrome DevTools Protocol 的浏览器动作后端。
///
/// 当前仅声明浏览器选择器的支持范围，实际 CDP 通信尚未实现。
pub struct CdpBackend;

#[async_trait]
impl ActionBackend for CdpBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::BrowserCdp
    }

    fn supports(&self, action: &AutomationAction) -> bool {
        matches!(
            action,
            AutomationAction::Click {
                target: Selector::Browser { .. }
            } | AutomationAction::SetValue {
                target: Selector::Browser { .. },
                ..
            }
        )
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: self.kind(),
            message: "Chrome DevTools Protocol 尚未接入".to_owned(),
        })
    }
}
