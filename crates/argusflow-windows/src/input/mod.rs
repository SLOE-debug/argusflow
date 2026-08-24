//! Windows 输入事件注入后端。

use argusflow_agent::{ActionBackend, ActionCapability};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{QueryCost, SupportLevel};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 使用 Windows `SendInput` 注入坐标动作的后端。
pub struct SendInputBackend;

#[async_trait]
impl ActionBackend for SendInputBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::SendInput
    }

    fn plan(&self, action: &AutomationAction) -> ActionCapability {
        if matches!(&action.target().locator, TargetLocator::Coordinate { .. }) {
            ActionCapability {
                level: SupportLevel::Native,
                estimated_cost: QueryCost::Low,
            }
        } else {
            ActionCapability::unsupported()
        }
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: self.kind(),
            message: "SendInput 兜底尚未接入".to_owned(),
        })
    }
}
