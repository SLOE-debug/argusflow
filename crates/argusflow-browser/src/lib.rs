//! 浏览器自动化后端及其 Chrome DevTools Protocol 接入点。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

/// Chrome DevTools Protocol 查询规划能力。
pub mod cdp;

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PreparedCandidate,
    PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{analyze_query, parse_stored_query};
use async_trait::async_trait;

use cdp::{CdpQueryPlan, compile_cdp_query};

#[derive(Debug, Default)]
/// 基于 Chrome DevTools Protocol 的浏览器动作后端。
///
/// 当前能够分析 AQL/原生 CSS 支持范围，实际 CDP 通信尚未实现。
pub struct CdpBackend;

impl ActionBackend for CdpBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::BrowserCdp
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection> {
        let TargetLocator::Query { query } = &action.target().locator else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::BrowserCdp,
            });
        };
        let parsed = parse_stored_query(query).map_err(|_| PlanRejection::InvalidAction {
            backend: BackendKind::BrowserCdp,
        })?;
        let portability = analyze_query(&parsed).portability().clone();
        let query_plan = compile_cdp_query(&parsed).map_err(|_| PlanRejection::Unsupported {
            backend: BackendKind::BrowserCdp,
        })?;
        let explain = PlanExplain {
            backend: BackendKind::BrowserCdp,
            earliest_supported_branch_index: Some(
                query_plan.capability.earliest_supported_branch_index,
            ),
            support: query_plan.capability.level,
            cost: query_plan.capability.estimated_cost,
            availability: RuntimeAvailability::NotImplemented,
            context_fitness: cdp_context_fitness(context),
            portability,
            steps: cdp::explain_cdp_plan(&query_plan.expression),
            diagnostics: query_plan.diagnostics.clone(),
        };
        let execution = CdpPreparedExecution {
            action: action.clone(),
            query_plan,
        };
        Ok(PreparedCandidate::new(explain, Arc::new(execution)))
    }
}

/// 已绑定动作与 CDP compiler plan 的执行实例。
#[derive(Debug)]
struct CdpPreparedExecution {
    /// 准备阶段冻结的动作参数。
    action: AutomationAction,
    /// 准备阶段冻结的 CDP 逻辑计划。
    query_plan: CdpQueryPlan,
}

#[async_trait]
impl PreparedExecution for CdpPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let _prepared = (&self.action, &self.query_plan);
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::BrowserCdp,
            message: "Chrome DevTools Protocol 尚未接入".to_owned(),
        })
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
