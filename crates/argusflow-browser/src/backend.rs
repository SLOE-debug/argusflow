//! ActionBackend prepare、availability 与冻结 CDP execution 装配。

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PreparedCandidate,
    PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, PreparedAutomationTarget,
    PreparedTargetLocator, TargetLocator, UiQuery,
};
use argusflow_query::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend, analyze_query,
    canonicalize_query, parse_stored_query,
};
use async_trait::async_trait;

use crate::cdp::{
    CdpPageSession, CdpQueryPlan, CdpSessionRegistry, compile_cdp_query, execute_cdp_action,
    explain_cdp_plan,
};

/// 使用应用级持久 CDP session registry 的浏览器动作后端。
#[derive(Debug)]
pub struct CdpBackend {
    /// Browser 资源获取阶段注册、清理阶段移除的页面会话。
    sessions: Arc<CdpSessionRegistry>,
}

impl CdpBackend {
    /// 创建绑定唯一应用级 CDP runtime 的后端。
    pub fn new(runtime: &crate::CdpRuntime) -> Self {
        Self {
            sessions: runtime.sessions.clone(),
        }
    }
}

impl ActionBackend for CdpBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::BrowserCdp
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        if matches!(
            action,
            AutomationAction::PressKey { .. } | AutomationAction::TypeText { .. }
        ) {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::BrowserCdp,
            });
        }
        let TargetLocator::Query { query } = &action.target().locator else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::BrowserCdp,
            });
        };
        let parsed = parse_stored_query(query).map_err(|_| PlanRejection::InvalidAction {
            backend: BackendKind::BrowserCdp,
        })?;
        self.prepare_query(action, context, &parsed, canonicalize_query(&parsed))
    }

    fn prepare_with_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let Some(PreparedTargetLocator::Query { query, source }) =
            prepared_target.map(PreparedAutomationTarget::locator)
        else {
            return self.prepare(action, context);
        };
        self.prepare_query(action, context, query, source.clone())
    }
}

impl CdpBackend {
    /// 从 Runtime 已绑定参数的查询构建 CDP 候选，避免后端重新解析持久化源码。
    fn prepare_query(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        parsed: &UiQuery,
        query: String,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let portability = analyze_query(parsed).portability().clone();
        let query_plans = compile_cdp_query(parsed).map_err(|_| PlanRejection::Unsupported {
            backend: BackendKind::BrowserCdp,
        })?;
        let page_session = context
            .browser_session
            .as_ref()
            .and_then(|context_session| {
                self.sessions
                    .get(context_session.session_id)
                    .filter(|session| session.target_id() == context_session.target_id)
            });
        let availability = if page_session.is_some() {
            RuntimeAvailability::Ready
        } else {
            RuntimeAvailability::MissingContext
        };
        let candidates = query_plans
            .into_iter()
            .map(|plan| {
                let mut diagnostics = plan.diagnostics.clone();
                if !availability.is_ready() {
                    diagnostics.push(Diagnostic::global(
                        DiagnosticCode::RuntimeUnavailable,
                        DiagnosticSeverity::Information,
                        Some(QueryBackend::BrowserCdp),
                    ));
                }
                let explain = PlanExplain {
                    backend: BackendKind::BrowserCdp,
                    branch_path: Some(plan.capability.branch_path.clone()),
                    support: plan.capability.level,
                    cost: plan.capability.estimated_cost,
                    availability,
                    context_fitness: cdp_context_fitness(context),
                    portability: portability.clone(),
                    steps: explain_cdp_plan(&plan.expression),
                    diagnostics,
                };
                let execution = CdpPreparedExecution {
                    action: action.clone(),
                    page_session: page_session.clone(),
                    query_plan: plan,
                    query: query.clone(),
                };
                PreparedCandidate::new(explain, Arc::new(execution))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            Err(PlanRejection::Unsupported {
                backend: BackendKind::BrowserCdp,
            })
        } else {
            Ok(candidates)
        }
    }
}

/// 已绑定动作、CSS 查询计划与页面会话的执行实例。
#[derive(Debug)]
struct CdpPreparedExecution {
    /// 准备阶段冻结的动作参数。
    action: AutomationAction,
    /// 准备阶段冻结的可选 page session。
    page_session: Option<Arc<CdpPageSession>>,
    /// 准备阶段冻结的 CDP 逻辑计划。
    query_plan: CdpQueryPlan,
    /// 规范化查询，用于稳定错误信息。
    query: String,
}

#[async_trait]
impl PreparedExecution for CdpPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let page_session =
            self.page_session
                .as_ref()
                .ok_or_else(|| AutomationError::BackendUnavailable {
                    backend: BackendKind::BrowserCdp,
                    message: "prepared CDP action has no attached browser session".to_owned(),
                })?;
        execute_cdp_action(
            page_session,
            &self.action,
            &self.query_plan.expression,
            &self.query,
        )
        .await
    }
}

/// 评估 CDP 会话与当前浏览器上下文的匹配度。
fn cdp_context_fitness(context: &ExecutionContext) -> ContextFitness {
    match &context.browser_session {
        Some(session) if session.attached => ContextFitness::Excellent,
        Some(_) => ContextFitness::Poor,
        None => ContextFitness::Neutral,
    }
}
