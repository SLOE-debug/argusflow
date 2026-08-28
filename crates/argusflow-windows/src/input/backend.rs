use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection,
    PlanStepExplain, PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability,
    WindowContext,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, KeyChord, TargetLocator,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;

use super::keyboard::{inject_chord, inject_text};

/// 使用 Windows `SendInput` 向已验证前台窗口注入键盘事件的后端。
#[derive(Debug, Default)]
pub struct SendInputBackend;

impl ActionBackend for SendInputBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::SendInput
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let plan = match action {
            AutomationAction::PressKey { target, chord }
                if matches!(&target.locator, TargetLocator::Focused) =>
            {
                SendInputPlan::PressKey {
                    chord: chord.clone(),
                }
            }
            AutomationAction::TypeText { target, value }
                if matches!(&target.locator, TargetLocator::Focused) =>
            {
                SendInputPlan::TypeText {
                    value: value.clone(),
                }
            }
            AutomationAction::Click { target }
                if matches!(&target.locator, TargetLocator::Coordinate { .. }) =>
            {
                return coordinate_click_candidate(action, context);
            }
            _ => {
                return Err(PlanRejection::Unsupported {
                    backend: BackendKind::SendInput,
                });
            }
        };
        let window = context.foreground_window.clone();
        let availability = if window.is_some() {
            RuntimeAvailability::Ready
        } else {
            RuntimeAvailability::MissingContext
        };
        let explain = PlanExplain {
            backend: BackendKind::SendInput,
            branch_path: Some(BranchPath::root()),
            support: SupportLevel::Native,
            cost: QueryCost::Low,
            availability,
            context_fitness: if window.is_some() {
                ContextFitness::Good
            } else {
                ContextFitness::Poor
            },
            portability: QueryPortability::Portable,
            steps: vec![PlanStepExplain {
                kind: PlanStepKind::Action,
                summary: plan.summary().to_owned(),
            }],
            diagnostics: Vec::new(),
        };
        Ok(vec![PreparedCandidate::new(
            explain,
            Arc::new(SendInputPreparedExecution { plan, window }),
        )])
    }
}

/// 已冻结且不再解释工作流字段的输入动作。
#[derive(Debug, Clone)]
enum SendInputPlan {
    /// 单次组合键。
    PressKey { chord: KeyChord },
    /// Unicode 文本输入。
    TypeText { value: String },
}

impl SendInputPlan {
    /// 返回 Explain 使用的稳定动作摘要。
    const fn summary(&self) -> &'static str {
        match self {
            Self::PressKey { .. } => "foreground keyboard chord via SendInput",
            Self::TypeText { .. } => "foreground Unicode text via SendInput",
        }
    }
}

/// 已绑定窗口身份和输入负载的执行计划。
#[derive(Debug)]
struct SendInputPreparedExecution {
    /// 准备阶段冻结的输入动作。
    plan: SendInputPlan,
    /// AppSession 或当前前台窗口提供的 HWND/PID。
    window: Option<WindowContext>,
}

#[async_trait]
impl PreparedExecution for SendInputPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| AutomationError::BackendUnavailable {
                backend: BackendKind::SendInput,
                message: "prepared keyboard input has no window context".to_owned(),
            })?;
        let result = match &self.plan {
            SendInputPlan::PressKey { chord } => inject_chord(window, chord),
            SendInputPlan::TypeText { value } => inject_text(window, value),
        };
        result.map_err(|error| AutomationError::BackendFailed {
            backend: BackendKind::SendInput,
            message: error.to_string(),
        })?;
        Ok(ActionOutcome {
            backend: BackendKind::SendInput,
            message: match &self.plan {
                SendInputPlan::PressKey { .. } => "已向目标窗口发送组合键",
                SendInputPlan::TypeText { .. } => "已向目标窗口输入文本",
            }
            .to_owned(),
            outputs: Default::default(),
            diagnostic_evidence: Vec::new(),
        })
    }
}

/// 保留已有坐标点击候选，但在鼠标输入实现前继续准确报告 NotImplemented。
fn coordinate_click_candidate(
    action: &AutomationAction,
    context: &ExecutionContext,
) -> Result<Vec<PreparedCandidate>, PlanRejection> {
    let TargetLocator::Coordinate { point } = &action.target().locator else {
        return Err(PlanRejection::Unsupported {
            backend: BackendKind::SendInput,
        });
    };
    let explain = PlanExplain {
        backend: BackendKind::SendInput,
        branch_path: Some(BranchPath::root()),
        support: SupportLevel::Native,
        cost: QueryCost::Low,
        availability: RuntimeAvailability::NotImplemented,
        context_fitness: if context.foreground_window.is_some() {
            ContextFitness::Good
        } else {
            ContextFitness::Neutral
        },
        portability: QueryPortability::Portable,
        steps: vec![PlanStepExplain {
            kind: PlanStepKind::Action,
            summary: format!("screen point ({}, {})", point.x, point.y),
        }],
        diagnostics: Vec::new(),
    };
    Ok(vec![PreparedCandidate::new(
        explain,
        Arc::new(UnimplementedCoordinateClick),
    )])
}

/// 鼠标输入尚未接入时使用的显式占位执行器。
#[derive(Debug)]
struct UnimplementedCoordinateClick;

#[async_trait]
impl PreparedExecution for UnimplementedCoordinateClick {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::SendInput,
            message: "SendInput 鼠标点击尚未接入".to_owned(),
        })
    }
}
