//! 单一事实源选择、Unknown fallback 与 AQL v3 统一求值。

use std::sync::Arc;

use argusflow_core::{
    AutomationExecutionScope, BackendKind, ObservationRequest, ObservationResult,
    ObservationUnknownReason,
};
use argusflow_query::evaluate_observation;
use argusflow_runtime::ObservationDispatcher;
use async_trait::async_trait;

use crate::{
    BrowserSessionContext, ExecutionContextProvider, ObservationBackend, ObservationBackendError,
    StaticExecutionContext, WindowContext,
};

/// 按策略和稳定后端顺序执行观察，不跨后端合并实体事实。
pub struct ObservationRouter {
    /// 已注册的观察事实源。
    backends: Vec<Arc<dyn ObservationBackend>>,
    /// 每次观察前捕获最新运行上下文。
    context_provider: Arc<dyn ExecutionContextProvider>,
}

impl ObservationRouter {
    /// 使用空上下文创建路由器，主要供测试或显式作用域使用。
    pub fn new(backends: Vec<Arc<dyn ObservationBackend>>) -> Self {
        Self {
            backends,
            context_provider: Arc::new(StaticExecutionContext::default()),
        }
    }

    /// 创建使用宿主最新上下文快照的观察路由器。
    pub fn with_context_provider(
        backends: Vec<Arc<dyn ObservationBackend>>,
        context_provider: Arc<dyn ExecutionContextProvider>,
    ) -> Self {
        Self {
            backends,
            context_provider,
        }
    }

    /// 返回策略偏好与稳定默认顺序组成的后端排序键。
    fn rank(&self, request: &ObservationRequest, backend: BackendKind) -> (usize, usize) {
        let stable = crate::ROUTE_TIE_BREAK_ORDER
            .iter()
            .position(|candidate| *candidate == backend)
            .unwrap_or(crate::ROUTE_TIE_BREAK_ORDER.len());
        (request.backend_policy.preference_rank(backend), stable)
    }
}

#[async_trait]
impl ObservationDispatcher for ObservationRouter {
    async fn observe(
        &self,
        request: &ObservationRequest,
        scope: AutomationExecutionScope,
    ) -> ObservationResult {
        let mut context = self.context_provider.snapshot();
        apply_scope(&mut context, scope);
        let mut backends = self
            .backends
            .iter()
            .filter(|backend| request.backend_policy.allows(backend.kind()))
            .collect::<Vec<_>>();
        backends.sort_by_key(|backend| self.rank(request, backend.kind()));

        let mut last_unknown = ObservationResult::Unknown {
            backend: None,
            reason: ObservationUnknownReason::BackendUnavailable,
            retryable: false,
        };
        for backend in backends {
            let kind = backend.kind();
            match backend.observe(request, &context).await {
                Ok(observations) => {
                    let result = evaluate_observation(&request.query, &observations, kind);
                    if matches!(result, ObservationResult::Known { .. }) {
                        return result;
                    }
                    last_unknown = result;
                }
                Err(ObservationBackendError::Unsupported) => {}
                Err(ObservationBackendError::Unavailable { retryable }) => {
                    last_unknown = ObservationResult::Unknown {
                        backend: Some(kind),
                        reason: ObservationUnknownReason::BackendUnavailable,
                        retryable,
                    };
                }
                Err(ObservationBackendError::Unknown { reason, retryable }) => {
                    last_unknown = ObservationResult::Unknown {
                        backend: Some(kind),
                        reason,
                        retryable,
                    };
                }
            }
        }
        last_unknown
    }
}

/// 将 Runtime 已解析作用域投影到 Agent 的最新上下文快照。
fn apply_scope(context: &mut crate::ExecutionContext, scope: AutomationExecutionScope) {
    match scope {
        AutomationExecutionScope::Current => {}
        AutomationExecutionScope::Window {
            handle,
            process_id,
            capabilities,
        } => {
            context.foreground_window = Some(WindowContext { handle, process_id });
            context.active_process = None;
            context.browser_session = None;
            context.visual_cache.ready = false;
            if !capabilities.contains(&argusflow_core::CapabilityId::WINDOWS_UIA) {
                context.accessibility.ready = false;
            }
        }
        AutomationExecutionScope::Browser {
            session_id,
            target_id,
        } => {
            context.foreground_window = None;
            context.active_process = None;
            context.browser_session = Some(BrowserSessionContext {
                session_id,
                target_id,
                attached: true,
            });
            context.accessibility.ready = false;
            context.visual_cache.ready = false;
        }
    }
}
