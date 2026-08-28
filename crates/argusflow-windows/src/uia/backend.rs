//! ActionBackend prepare、availability 与冻结 UIA execution 的装配。

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, EvidenceBundle, EvidenceCaptureError, EvidenceCaptureRequest,
    ExecutionContext, PlanExplain, PlanRejection, PreparedCandidate, PreparedDiagnostics,
    PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend, analyze_query,
    canonicalize_query, parse_stored_query,
};
use async_trait::async_trait;

use super::{
    action_compiler::compile_uia_action,
    compiler::compile_uia_query,
    explain::{explain_uia_action, explain_uia_plan},
    plan::UiaPreparedPlan,
    runtime::{
        PreparedWindowTarget, UiaEvidenceRequest, UiaExecuteRequest, UiaRuntime, UiaRuntimeState,
    },
};

/// 使用共享 `UiaRuntime` 操作原生 Windows UIA provider 的后端。
#[derive(Debug)]
pub struct UiaBackend {
    /// 与 Windows ExecutionContextProvider 共享的唯一 UIA runtime。
    runtime: Arc<UiaRuntime>,
}

impl UiaBackend {
    /// 创建绑定应用级 UIA runtime 的后端。
    pub fn new(runtime: Arc<UiaRuntime>) -> Self {
        Self { runtime }
    }
}

impl ActionBackend for UiaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let (query, window) = match &action.target().locator {
            TargetLocator::Query { query } => (
                query,
                context
                    .foreground_window
                    .as_ref()
                    .map(|window| PreparedWindowTarget {
                        handle: window.handle,
                        process_id: window.process_id,
                    }),
            ),
            TargetLocator::Visual { .. }
            | TargetLocator::Coordinate { .. }
            | TargetLocator::Focused => {
                return Err(PlanRejection::Unsupported {
                    backend: BackendKind::WindowsUia,
                });
            }
        };
        let parsed = parse_stored_query(query).map_err(|_| PlanRejection::InvalidAction {
            backend: BackendKind::WindowsUia,
        })?;
        let portability = analyze_query(&parsed).portability().clone();
        let query_plans = compile_uia_query(&parsed).map_err(|_| PlanRejection::Unsupported {
            backend: BackendKind::WindowsUia,
        })?;
        let availability = runtime_availability(self.runtime.health().snapshot(), window);
        let query = canonicalize_query(&parsed);
        let mut candidates = Vec::new();
        for query_plan in query_plans {
            let Ok(prepared_plan) = compile_uia_action(action, query_plan) else {
                continue;
            };
            let mut diagnostics = prepared_plan.query.diagnostics.clone();
            if !availability.is_ready() {
                diagnostics.push(Diagnostic::global(
                    DiagnosticCode::RuntimeUnavailable,
                    DiagnosticSeverity::Information,
                    Some(QueryBackend::WindowsUia),
                ));
            }
            let mut steps = explain_uia_plan(&prepared_plan.query.expression);
            steps.push(explain_uia_action(
                &prepared_plan.action,
                prepared_plan.action_support,
            ));
            let explain = PlanExplain {
                backend: BackendKind::WindowsUia,
                branch_path: Some(prepared_plan.capability.branch_path.clone()),
                support: prepared_plan.capability.level,
                cost: prepared_plan.capability.estimated_cost,
                availability,
                context_fitness: uia_context_fitness(context),
                portability: portability.clone(),
                steps,
                diagnostics,
            };
            let execution = UiaPreparedExecution {
                runtime: self.runtime.clone(),
                window,
                plan: prepared_plan.clone(),
                query: query.clone(),
            };
            let diagnostics = UiaPreparedDiagnostics {
                runtime: self.runtime.clone(),
                window,
                plan: prepared_plan,
                query: query.clone(),
            };
            candidates.push(
                PreparedCandidate::new(explain, Arc::new(execution))
                    .with_diagnostics(Arc::new(diagnostics)),
            );
        }
        if candidates.is_empty() {
            Err(PlanRejection::Unsupported {
                backend: BackendKind::WindowsUia,
            })
        } else {
            Ok(candidates)
        }
    }
}

/// 已绑定冻结查询、动作与窗口上下文的 UIA 诊断对象。
#[derive(Debug)]
struct UiaPreparedDiagnostics {
    /// 与 execution 共用、保证 COM apartment 的 UIA runtime。
    runtime: Arc<UiaRuntime>,
    /// prepare 时冻结的 HWND/PID。
    window: Option<PreparedWindowTarget>,
    /// 失败 candidate 的完整 UIA prepared plan。
    plan: UiaPreparedPlan,
    /// 规范化查询。
    query: String,
}

#[async_trait]
impl PreparedDiagnostics for UiaPreparedDiagnostics {
    fn backend(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    async fn capture(
        &self,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        let window = self
            .window
            .ok_or_else(|| EvidenceCaptureError::SourceUnavailable {
                message: "prepared UIA diagnostics has no window context".to_owned(),
            })?;
        self.runtime
            .capture(UiaEvidenceRequest {
                window,
                plan: self.plan.clone(),
                query: self.query.clone(),
                capture: request,
            })
            .await
    }
}

/// 已绑定窗口、动作与 UIA compiler plan 的执行实例。
#[derive(Debug)]
struct UiaPreparedExecution {
    /// prepare 与 execute 共享的专用 UIA runtime。
    runtime: Arc<UiaRuntime>,
    /// prepare 时冻结的前台或 AppSession HWND/PID。
    window: Option<PreparedWindowTarget>,
    /// prepare 时冻结的 UIA 查询、动作与联合能力计划。
    plan: UiaPreparedPlan,
    /// 规范化查询，仅用于错误日志。
    query: String,
}

#[async_trait]
impl PreparedExecution for UiaPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let window = self
            .window
            .ok_or_else(|| AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: "prepared UIA candidate has no window context".to_owned(),
            })?;
        self.runtime
            .execute(UiaExecuteRequest {
                window,
                plan: self.plan.clone(),
                query: self.query.clone(),
            })
            .await
    }
}

/// 组合 runtime health 与准备阶段窗口上下文。
fn runtime_availability(
    runtime: UiaRuntimeState,
    window: Option<PreparedWindowTarget>,
) -> RuntimeAvailability {
    match runtime {
        UiaRuntimeState::Ready if window.is_some() => RuntimeAvailability::Ready,
        UiaRuntimeState::Ready => RuntimeAvailability::MissingContext,
        UiaRuntimeState::Initializing
        | UiaRuntimeState::InitializationFailed { .. }
        | UiaRuntimeState::Stopped => RuntimeAvailability::Unavailable,
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
