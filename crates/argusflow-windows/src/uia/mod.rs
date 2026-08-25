//! Windows UI Automation 后端。

mod compiler;
mod explain;
mod plan;

pub use compiler::{UiaQueryCompileError, compile_uia_query};
pub use plan::{UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan};

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

use self::explain::explain_uia_plan;

#[derive(Debug, Default)]
/// 使用 Windows UI Automation 操作原生控件的后端。
pub struct UiaBackend;

impl ActionBackend for UiaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection> {
        let TargetLocator::Query { query } = &action.target().locator else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::WindowsUia,
            });
        };
        let parsed = parse_stored_query(query).map_err(|_| PlanRejection::InvalidAction {
            backend: BackendKind::WindowsUia,
        })?;
        let portability = analyze_query(&parsed).portability().clone();
        let query_plan = compile_uia_query(&parsed).map_err(|_| PlanRejection::Unsupported {
            backend: BackendKind::WindowsUia,
        })?;
        let explain = PlanExplain {
            backend: BackendKind::WindowsUia,
            support: query_plan.capability.level,
            cost: query_plan.capability.estimated_cost,
            availability: RuntimeAvailability::NotImplemented,
            context_fitness: uia_context_fitness(context),
            portability,
            steps: explain_uia_plan(&query_plan.expression),
            diagnostics: query_plan.diagnostics.clone(),
        };
        let execution = UiaPreparedExecution {
            action: action.clone(),
            query_plan,
        };
        Ok(PreparedCandidate::new(explain, Arc::new(execution)))
    }
}

/// 已绑定动作与 UIA compiler plan 的执行实例。
#[derive(Debug)]
struct UiaPreparedExecution {
    /// 准备阶段冻结的动作参数。
    action: AutomationAction,
    /// 准备阶段冻结的 UIA 逻辑计划。
    query_plan: UiaQueryPlan,
}

#[async_trait]
impl PreparedExecution for UiaPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let _prepared = (&self.action, &self.query_plan);
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::WindowsUia,
            message: "Windows UI Automation 尚未接入".to_owned(),
        })
    }
}

/// 评估 UIA 与当前前台窗口及 Accessibility 状态的匹配度。
const fn uia_context_fitness(context: &ExecutionContext) -> ContextFitness {
    if context.accessibility.ready && context.foreground_window.is_some() {
        ContextFitness::Good
    } else if context.foreground_window.is_some() {
        ContextFitness::Neutral
    } else {
        ContextFitness::Poor
    }
}
