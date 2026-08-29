use std::{fmt, sync::Arc};

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, MaterializedTarget,
    MaterializedTargetValidator, PlanExplain, PlanRejection, PlanStepExplain, PlanStepKind,
    PreparedCandidate, PreparedExecution, RuntimeAvailability, WindowContext,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, KeyChord, PreparedTargetLocator,
    ScreenPoint, TargetLocator,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;

use super::{
    keyboard::{ensure_foreground_window, inject_chord, inject_text},
    mouse::{inject_click, inject_click_with_surface},
};

/// 使用 Windows `SendInput` 向已验证前台窗口注入键盘事件的后端。
pub struct SendInputBackend {
    /// 与视觉物化器共享的输入前最后一刻新鲜度复验器。
    target_validator: Option<Arc<dyn MaterializedTargetValidator>>,
}

impl fmt::Debug for SendInputBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendInputBackend")
            .field("has_target_validator", &self.target_validator.is_some())
            .finish()
    }
}

impl Default for SendInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SendInputBackend {
    /// 创建只负责物理输入注入的后端。
    pub const fn new() -> Self {
        Self {
            target_validator: None,
        }
    }

    /// 创建带有视觉目标输入前复验能力的 SendInput 后端。
    pub fn with_target_validator(validator: Arc<dyn MaterializedTargetValidator>) -> Self {
        Self {
            target_validator: Some(validator),
        }
    }
}

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
            Arc::new(SendInputPreparedExecution {
                plan,
                window,
                target_validator: None,
            }),
        )])
    }

    fn prepare_with_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&argusflow_core::PreparedAutomationTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        self.prepare_with_materialized_target(action, context, prepared_target, None)
    }

    fn prepare_with_materialized_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&argusflow_core::PreparedAutomationTarget>,
        materialized_target: Option<&MaterializedTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let AutomationAction::Click { target } = action else {
            return self.prepare(action, context);
        };
        if !matches!(&target.locator, TargetLocator::Visual { .. }) {
            return self.prepare(action, context);
        }
        let Some(prepared_target) = prepared_target else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::SendInput,
            });
        };
        let PreparedTargetLocator::Visual { query } = prepared_target.locator() else {
            return Err(PlanRejection::Unsupported {
                backend: BackendKind::SendInput,
            });
        };
        let window = context.foreground_window.clone();
        let availability = match (window.as_ref(), materialized_target) {
            (None, _) => RuntimeAvailability::MissingContext,
            (Some(_), Some(materialized)) if window.as_ref() == Some(&materialized.window) => {
                RuntimeAvailability::Ready
            }
            (Some(_), Some(_)) | (Some(_), None) => RuntimeAvailability::Unavailable,
        };
        let stage_summary = materialized_target
            .map(|target| format!("planner materializer selected {:?}", target.source_backend))
            .unwrap_or_else(|| "planner materializer pending: cache -> tiny -> medium".to_owned());
        let explain = PlanExplain {
            backend: BackendKind::SendInput,
            branch_path: Some(BranchPath::root()),
            support: SupportLevel::Native,
            cost: QueryCost::High,
            availability,
            context_fitness: if window.is_some() {
                ContextFitness::Good
            } else {
                ContextFitness::Poor
            },
            portability: QueryPortability::Portable,
            steps: vec![
                PlanStepExplain {
                    kind: PlanStepKind::Scope,
                    summary: "frozen AppSession HWND/PID visual scope".to_owned(),
                },
                PlanStepExplain {
                    kind: PlanStepKind::Cache,
                    summary: stage_summary,
                },
                PlanStepExplain {
                    kind: PlanStepKind::CandidateSource,
                    summary: format!("visual target query: {:?}", query.text),
                },
                PlanStepExplain {
                    kind: PlanStepKind::Action,
                    summary: "foreground mouse click via SendInput".to_owned(),
                },
            ],
            diagnostics: Vec::new(),
        };
        Ok(vec![PreparedCandidate::new(
            explain,
            Arc::new(SendInputPreparedExecution {
                plan: SendInputPlan::VisualClick {
                    materialized_target: materialized_target.cloned(),
                },
                window,
                target_validator: self.target_validator.clone(),
            }),
        )])
    }
}

/// 已冻结且不再解释工作流字段的输入动作。
#[derive(Debug, Clone)]
enum SendInputPlan {
    /// 已由调用方提供并在执行时再次复验的屏幕坐标点击。
    Click { point: ScreenPoint },
    /// 使用 Planner 已冻结的视觉目标事实执行的物理点击。
    VisualClick {
        /// Planner 已经物化并绑定 scene/frame/generation 的屏幕目标。
        materialized_target: Option<MaterializedTarget>,
    },
    /// 单次组合键。
    PressKey { chord: KeyChord },
    /// Unicode 文本输入。
    TypeText { value: String },
}

impl SendInputPlan {
    /// 返回 Explain 使用的稳定动作摘要。
    const fn summary(&self) -> &'static str {
        match self {
            Self::Click { .. } => "foreground mouse click via SendInput",
            Self::VisualClick { .. } => {
                "Vision materialize then foreground mouse click via SendInput"
            }
            Self::PressKey { .. } => "foreground keyboard chord via SendInput",
            Self::TypeText { .. } => "foreground Unicode text via SendInput",
        }
    }
}

/// 已绑定窗口身份和输入负载的执行计划。
struct SendInputPreparedExecution {
    /// 准备阶段冻结的输入动作。
    plan: SendInputPlan,
    /// AppSession 或当前前台窗口提供的 HWND/PID。
    window: Option<WindowContext>,
    /// 视觉点击在注入前调用的最新 scene/topology 复验器。
    target_validator: Option<Arc<dyn MaterializedTargetValidator>>,
}

impl fmt::Debug for SendInputPreparedExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendInputPreparedExecution")
            .field("plan", &self.plan)
            .field("window", &self.window)
            .field("has_target_validator", &self.target_validator.is_some())
            .finish()
    }
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
        let result: Result<(), AutomationError> = match &self.plan {
            SendInputPlan::PressKey { chord } => {
                inject_chord(window, chord).map_err(|error| AutomationError::BackendFailed {
                    backend: BackendKind::SendInput,
                    message: error.to_string(),
                })
            }
            SendInputPlan::TypeText { value } => {
                inject_text(window, value).map_err(|error| AutomationError::BackendFailed {
                    backend: BackendKind::SendInput,
                    message: error.to_string(),
                })
            }
            SendInputPlan::Click { point } => {
                inject_click(window, *point).map_err(|error| AutomationError::BackendFailed {
                    backend: BackendKind::SendInput,
                    message: error.to_string(),
                })
            }
            SendInputPlan::VisualClick {
                materialized_target,
            } => {
                let materialized_target = materialized_target.as_ref().ok_or_else(|| {
                    AutomationError::BackendUnavailable {
                        backend: BackendKind::VisualCache,
                        message: "planner did not provide a materialized visual target".to_owned(),
                    }
                })?;
                ensure_foreground_window(window).map_err(|error| {
                    AutomationError::BackendFailed {
                        backend: BackendKind::SendInput,
                        message: error.to_string(),
                    }
                })?;
                if materialized_target.window != *window {
                    return Err(AutomationError::VisualTargetStale {
                        message: "visual materializer returned a different window identity"
                            .to_owned(),
                    });
                }
                if !contains_point(materialized_target.bounds, materialized_target.safe_point) {
                    return Err(AutomationError::BackendFailed {
                        backend: BackendKind::SendInput,
                        message: "materialized visual target point is outside its bounds"
                            .to_owned(),
                    });
                }
                let validator = self.target_validator.as_ref().ok_or_else(|| {
                    AutomationError::BackendUnavailable {
                        backend: BackendKind::SendInput,
                        message: "visual click has no input freshness validator".to_owned(),
                    }
                })?;
                validator.validate_before_input(materialized_target).await?;
                inject_click_with_surface(
                    window,
                    materialized_target.safe_point,
                    Some(materialized_target.surface_bounds),
                )
                .map_err(|error| AutomationError::BackendFailed {
                    backend: BackendKind::SendInput,
                    message: error.to_string(),
                })
            }
        };
        result?;
        Ok(ActionOutcome {
            backend: BackendKind::SendInput,
            message: match &self.plan {
                SendInputPlan::PressKey { .. } => "已向目标窗口发送组合键",
                SendInputPlan::TypeText { .. } => "已向目标窗口输入文本",
                SendInputPlan::Click { .. } => "已向目标窗口执行坐标点击",
                SendInputPlan::VisualClick { .. } => "已通过统一视觉物化并执行鼠标点击",
            }
            .to_owned(),
            outputs: Default::default(),
            diagnostic_evidence: Vec::new(),
        })
    }
}

/// 复验物化目标的安全点仍位于其视觉 bbox 内，避免 stale target 变成越界输入。
const fn contains_point(bounds: argusflow_agent::VisualTargetBounds, point: ScreenPoint) -> bool {
    let right = bounds.x as i64 + bounds.width as i64;
    let bottom = bounds.y as i64 + bounds.height as i64;
    let x = point.x as i64;
    let y = point.y as i64;
    x >= bounds.x as i64 && x < right && y >= bounds.y as i64 && y < bottom
}

/// 构造带有前台窗口和坐标点的鼠标点击候选。
fn coordinate_click_candidate(
    action: &AutomationAction,
    context: &ExecutionContext,
) -> Result<Vec<PreparedCandidate>, PlanRejection> {
    let TargetLocator::Coordinate { point } = &action.target().locator else {
        return Err(PlanRejection::Unsupported {
            backend: BackendKind::SendInput,
        });
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
        context_fitness: if context.foreground_window.is_some() {
            ContextFitness::Good
        } else {
            ContextFitness::Neutral
        },
        portability: QueryPortability::Portable,
        steps: vec![PlanStepExplain {
            kind: PlanStepKind::Action,
            summary: format!(
                "screen point ({}, {}) via foreground SendInput",
                point.x, point.y
            ),
        }],
        diagnostics: Vec::new(),
    };
    Ok(vec![PreparedCandidate::new(
        explain,
        Arc::new(SendInputPreparedExecution {
            plan: SendInputPlan::Click { point: *point },
            window,
            target_validator: None,
        }),
    )])
}
