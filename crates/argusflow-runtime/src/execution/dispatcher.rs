use argusflow_core::{
    ActionExecutionOptions, ActionOutcome, AutomationAction, AutomationError,
    AutomationExecutionScope, BackendKind,
};
use async_trait::async_trait;

use argusflow_core::{ObservationRequest, ObservationResult, ObservationUnknownReason};

/// 将核心动作委托给具体自动化后端的异步接口。
#[async_trait]
pub trait ActionDispatcher: Send + Sync {
    /// 执行一个动作并返回后端结果或结构化自动化错误。
    async fn execute(
        &self,
        action: &AutomationAction,
        scope: AutomationExecutionScope,
    ) -> Result<ActionOutcome, AutomationError>;

    /// 使用节点提供的执行预算执行动作；默认分发器保持单次执行语义。
    async fn execute_with_options(
        &self,
        action: &AutomationAction,
        scope: AutomationExecutionScope,
        _options: ActionExecutionOptions,
    ) -> Result<ActionOutcome, AutomationError> {
        self.execute(action, scope).await
    }
}

/// 表示尚未配置任何可用自动化后端的占位实现。
#[derive(Debug, Default)]
pub struct UnavailableActionDispatcher;

#[async_trait]
impl ActionDispatcher for UnavailableActionDispatcher {
    /// 明确返回后端不可用，避免在未配置时静默跳过动作。
    async fn execute(
        &self,
        _action: &AutomationAction,
        _scope: AutomationExecutionScope,
    ) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::WindowsUia,
            message: "no automation backend has been configured".to_owned(),
        })
    }
}

/// 将一次冻结观察请求交给单一事实源路由器的异步边界。
#[async_trait]
pub trait ObservationDispatcher: Send + Sync {
    /// 执行一次观察；后端 fallback 与 Known 空结果权威语义由实现负责。
    async fn observe(
        &self,
        request: &ObservationRequest,
        scope: AutomationExecutionScope,
    ) -> ObservationResult;
}

/// 未装配观察后端时返回显式 Unknown 的占位实现。
#[derive(Debug, Default)]
pub struct UnavailableObservationDispatcher;

#[async_trait]
impl ObservationDispatcher for UnavailableObservationDispatcher {
    async fn observe(
        &self,
        _request: &ObservationRequest,
        _scope: AutomationExecutionScope,
    ) -> ObservationResult {
        ObservationResult::Unknown {
            backend: None,
            reason: ObservationUnknownReason::BackendUnavailable,
            retryable: false,
        }
    }
}
