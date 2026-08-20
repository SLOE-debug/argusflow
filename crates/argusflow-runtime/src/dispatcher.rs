use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind};
use async_trait::async_trait;

/// 将核心动作委托给具体自动化后端的异步接口。
#[async_trait]
pub trait ActionDispatcher: Send + Sync {
    /// 执行一个动作并返回后端结果或结构化自动化错误。
    async fn execute(&self, action: &AutomationAction) -> Result<ActionOutcome, AutomationError>;
}

/// 表示尚未配置任何可用自动化后端的占位实现。
#[derive(Debug, Default)]
pub struct UnavailableActionDispatcher;

#[async_trait]
impl ActionDispatcher for UnavailableActionDispatcher {
    /// 明确返回后端不可用，避免在未配置时静默跳过动作。
    async fn execute(&self, _action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::WindowsUia,
            message: "no automation backend has been configured".to_owned(),
        })
    }
}
