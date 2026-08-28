use std::{fmt, sync::Arc};

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PlanStepExplain,
    PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability, VisualResolvePolicy,
    VisualTargetResolver, WindowContext,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, KeyChord, ScreenPoint,
    TargetLocator,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;

use super::{
    keyboard::{ensure_foreground_window, inject_chord, inject_text},
    mouse::inject_click,
};

/// 使用 Windows `SendInput` 向已验证前台窗口注入键盘事件的后端。
#[derive(Default)]
pub struct SendInputBackend {
    /// Visual Click 所需的观察到坐标物化器；默认构造用于未装配视觉运行时的测试。
    resolver: Option<Arc<dyn VisualTargetResolver>>,
}

impl SendInputBackend {
    /// 创建绑定视觉目标解析器的物理输入后端。
    pub fn new(resolver: Arc<dyn VisualTargetResolver>) -> Self {
        Self {
            resolver: Some(resolver),
        }
    }
}

impl fmt::Debug for SendInputBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendInputBackend")
            .field("has_visual_resolver", &self.resolver.is_some())
            .finish()
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
            AutomationAction::Click { target }
                if matches!(&target.locator, TargetLocator::VisualResolved { .. }) =>
            {
                let TargetLocator::VisualResolved { query } = &target.locator else {
                    return Err(PlanRejection::Unsupported {
                        backend: BackendKind::SendInput,
                    });
                };
                SendInputPlan::VisualClick {
                    query: query.clone(),
                }
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
                resolver: self.resolver.clone(),
            }),
        )])
    }
}

/// 已冻结且不再解释工作流字段的输入动作。
#[derive(Debug, Clone)]
enum SendInputPlan {
    /// 已由调用方提供并在执行时再次复验的屏幕坐标点击。
    Click { point: ScreenPoint },
    /// 由当前稳定视觉场景解析安全点后执行的物理点击。
    VisualClick { query: argusflow_core::VisualQuery },
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
    /// 负责把已解析视觉文字转换为当前窗口安全点击点的窄接口。
    resolver: Option<Arc<dyn VisualTargetResolver>>,
}

impl fmt::Debug for SendInputPreparedExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SendInputPreparedExecution")
            .field("plan", &self.plan)
            .field("window", &self.window)
            .field("has_visual_resolver", &self.resolver.is_some())
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
            SendInputPlan::VisualClick { query } => {
                let resolver =
                    self.resolver
                        .as_ref()
                        .ok_or_else(|| AutomationError::BackendUnavailable {
                            backend: BackendKind::SendInput,
                            message: "visual click has no target resolver".to_owned(),
                        })?;
                ensure_foreground_window(window).map_err(|error| {
                    AutomationError::BackendFailed {
                        backend: BackendKind::SendInput,
                        message: error.to_string(),
                    }
                })?;
                let resolved = resolver
                    .resolve(window, query, VisualResolvePolicy::default())
                    .await?;
                if resolved.window != *window {
                    return Err(AutomationError::BackendFailed {
                        backend: BackendKind::SendInput,
                        message: "visual resolver returned a different window identity".to_owned(),
                    });
                }
                inject_click(window, resolved.safe_point).map_err(|error| {
                    AutomationError::BackendFailed {
                        backend: BackendKind::SendInput,
                        message: error.to_string(),
                    }
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
                SendInputPlan::VisualClick { .. } => "已通过视觉定位并执行鼠标点击",
            }
            .to_owned(),
            outputs: Default::default(),
            diagnostic_evidence: Vec::new(),
        })
    }
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
            resolver: None,
        }),
    )])
}
