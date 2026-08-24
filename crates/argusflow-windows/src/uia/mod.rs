//! Windows UI Automation 后端。

mod compiler;
mod plan;

pub use compiler::{UiaQueryCompileError, compile_uia_query};
pub use plan::{UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan};

use argusflow_agent::{ActionBackend, ActionCapability};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{QueryBackend, analyze_query, parse_stored_query};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 使用 Windows UI Automation 操作原生控件的后端。
pub struct UiaBackend;

#[async_trait]
impl ActionBackend for UiaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    fn plan(&self, action: &AutomationAction) -> ActionCapability {
        let TargetLocator::Query { query } = &action.target().locator else {
            return ActionCapability::unsupported();
        };
        let Ok(query) = parse_stored_query(query) else {
            return ActionCapability::unsupported();
        };
        let capability = analyze_query(&query).capability(QueryBackend::WindowsUia);
        ActionCapability {
            level: capability.level,
            estimated_cost: capability.estimated_cost,
        }
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: self.kind(),
            message: "Windows UI Automation 尚未接入".to_owned(),
        })
    }
}
