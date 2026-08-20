//! Windows UI Automation 后端。

use argusflow_agent::ActionBackend;
use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind, Selector};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 使用 Windows UI Automation 操作原生控件的后端。
pub struct UiaBackend;

#[async_trait]
impl ActionBackend for UiaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    fn supports(&self, action: &AutomationAction) -> bool {
        matches!(
            action,
            AutomationAction::Click {
                target: Selector::Native { .. }
            } | AutomationAction::SetValue {
                target: Selector::Native { .. },
                ..
            }
        )
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: self.kind(),
            message: "Windows UI Automation 尚未接入".to_owned(),
        })
    }
}
