//! 浏览器自动化后端及其 Chrome DevTools Protocol 接入点。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

/// Chrome DevTools Protocol 查询规划能力。
pub mod cdp;

use argusflow_agent::{ActionBackend, ActionCapability};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{QueryBackend, analyze_query, parse_stored_query};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 基于 Chrome DevTools Protocol 的浏览器动作后端。
///
/// 当前能够分析 AQL/原生 CSS 支持范围，实际 CDP 通信尚未实现。
pub struct CdpBackend;

#[async_trait]
impl ActionBackend for CdpBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::BrowserCdp
    }

    fn plan(&self, action: &AutomationAction) -> ActionCapability {
        let TargetLocator::Query { query } = &action.target().locator else {
            return ActionCapability::unsupported();
        };
        let Ok(query) = parse_stored_query(query) else {
            return ActionCapability::unsupported();
        };
        let capability = analyze_query(&query).capability(QueryBackend::BrowserCdp);
        ActionCapability {
            level: capability.level,
            estimated_cost: capability.estimated_cost,
        }
    }

    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: self.kind(),
            message: "Chrome DevTools Protocol 尚未接入".to_owned(),
        })
    }
}
