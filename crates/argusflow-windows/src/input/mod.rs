//! Windows 输入事件注入后端。

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PlanStepExplain,
    PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;

#[derive(Debug, Default)]
/// 使用 Windows `SendInput` 注入坐标动作的后端。
pub struct SendInputBackend;

impl ActionBackend for SendInputBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::SendInput
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        _context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection> {
        let TargetLocator::Coordinate { point } = &action.target().locator else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::SendInput,
            });
        };
        let explain = PlanExplain {
            backend: BackendKind::SendInput,
            earliest_supported_branch_index: Some(0),
            support: SupportLevel::Native,
            cost: QueryCost::Low,
            availability: RuntimeAvailability::NotImplemented,
            context_fitness: ContextFitness::Good,
            portability: QueryPortability::Portable,
            steps: vec![PlanStepExplain {
                kind: PlanStepKind::Action,
                summary: format!("screen point ({}, {})", point.x, point.y),
            }],
            diagnostics: Vec::new(),
        };
        Ok(PreparedCandidate::new(
            explain,
            Arc::new(SendInputPreparedExecution {
                action: action.clone(),
            }),
        ))
    }
}

/// 已绑定坐标与动作类型的 SendInput 执行计划。
#[derive(Debug)]
struct SendInputPreparedExecution {
    /// 准备阶段冻结的动作。
    action: AutomationAction,
}

#[async_trait]
impl PreparedExecution for SendInputPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let _prepared = &self.action;
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::SendInput,
            message: "SendInput 兜底尚未接入".to_owned(),
        })
    }
}
