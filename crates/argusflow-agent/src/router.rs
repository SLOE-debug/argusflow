use std::sync::Arc;

use argusflow_core::{
    ActionExecutionOptions, ActionOutcome, AutomationAction, AutomationError,
    AutomationExecutionScope, BackendKind, CapabilityId, PreparedAutomationTarget,
    PreparedVisualPostcondition, TargetScope, TargetWaitPolicy,
};
use argusflow_query::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryCost, QueryPortability, SupportLevel,
};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

use crate::visual_materialization;
use crate::{
    ActionBackend, ContextFitness, EvidenceSettings, ExecutionContext, ExecutionContextProvider,
    MaterializedTarget, PlanExplain, PlanRejection, PlanningReport, PreparedCandidate,
    PreparedPlan, PreparedTargetMaterializer, RuntimeAvailability, StaticExecutionContext,
    VisualVerificationProvider, VisualVerificationResult, WindowContext,
};

/// 能力、可用性、上下文和成本相同时使用的稳定兜底顺序。
pub const ROUTE_TIE_BREAK_ORDER: [BackendKind; 4] = [
    BackendKind::WindowsUia,
    BackendKind::BrowserCdp,
    BackendKind::OcrSmall,
    BackendKind::SendInput,
];

/// 根据最新 ExecutionContext 准备候选并冻结一次执行计划的动作分发器。
pub struct ActionRouter {
    /// 已注册后端实例。
    backends: Vec<Arc<dyn ActionBackend>>,
    /// 每次 prepare 前捕获最新上下文的提供器。
    context_provider: Arc<dyn ExecutionContextProvider>,
    /// PreparedPlan 使用的失败证据策略与 sink。
    evidence: EvidenceSettings,
    /// 非幂等输入动作使用的视觉基线/新事实 provider。
    visual_verification: Option<Arc<dyn VisualVerificationProvider>>,
    /// Planner 统一调用的视觉目标物化器；物理输入后端不再持有该依赖。
    target_materializer: Option<Arc<dyn PreparedTargetMaterializer>>,
}

impl ActionRouter {
    /// 使用空上下文创建路由器；宿主可通过 `with_context_provider` 注入真实快照。
    pub fn new(backends: Vec<Arc<dyn ActionBackend>>) -> Self {
        Self {
            backends,
            context_provider: Arc::new(StaticExecutionContext::default()),
            evidence: EvidenceSettings::default(),
            visual_verification: None,
            target_materializer: None,
        }
    }

    /// 创建使用指定运行上下文提供器的路由器。
    pub fn with_context_provider(
        backends: Vec<Arc<dyn ActionBackend>>,
        context_provider: Arc<dyn ExecutionContextProvider>,
    ) -> Self {
        Self {
            backends,
            context_provider,
            evidence: EvidenceSettings::default(),
            visual_verification: None,
            target_materializer: None,
        }
    }

    /// 为随后生成的 PreparedPlan 注入证据策略与宿主持久化边界。
    pub fn with_evidence(mut self, evidence: EvidenceSettings) -> Self {
        self.evidence = evidence;
        self
    }

    /// 注入与 SendInput 共用 VisionRuntime 的发送后验证 provider。
    pub fn with_visual_verification(
        mut self,
        provider: Arc<dyn VisualVerificationProvider>,
    ) -> Self {
        self.visual_verification = Some(provider);
        self
    }

    /// 注入由 Planner 统一管理的视觉目标物化器。
    pub fn with_target_materializer(
        mut self,
        materializer: Arc<dyn PreparedTargetMaterializer>,
    ) -> Self {
        self.target_materializer = Some(materializer);
        self
    }

    /// 返回稳定 tie-break 顺序，供开发者 Explain 展示。
    pub const fn route_tie_break_order(&self) -> &'static [BackendKind] {
        &ROUTE_TIE_BREAK_ORDER
    }

    /// 使用显式上下文准备并排序所有候选，返回只读 Planner 报告。
    pub fn inspect(&self, action: &AutomationAction, context: &ExecutionContext) -> PlanningReport {
        let mut explains = self
            .collect(action, context, None, None)
            .into_iter()
            .flat_map(|result| match result {
                Ok(candidates) => candidates
                    .into_iter()
                    .map(|candidate| candidate.explain().clone())
                    .collect(),
                Err(rejection) => vec![rejected_explain(rejection)],
            })
            .collect::<Vec<_>>();
        explains.sort_by_key(|explain| explain_rank(explain, &action.target().backend_policy));
        let selected_backend = explains
            .iter()
            .find(|explain| explain.support.is_supported() && explain.availability.is_ready())
            .map(|explain| explain.backend);
        PlanningReport {
            selected_backend,
            candidates: explains,
        }
    }

    /// 使用宿主提供器的最新上下文生成 Planner Explain。
    pub fn inspect_current(&self, action: &AutomationAction) -> PlanningReport {
        let mut context = self.context_provider.snapshot();
        if !matches!(&action.target().scope, TargetScope::Current) {
            context.foreground_window = None;
            context.active_process = None;
            context.browser_session = None;
            context.visual_cache.ready = false;
        }
        self.inspect(action, &context)
    }

    /// 使用显式上下文冻结可直接执行的 Ready 候选。
    pub fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedPlan, AutomationError> {
        self.prepare_with_target(action, context, None)
    }

    /// 使用 Runtime 已冻结的目标准备并排序可执行候选。
    pub fn prepare_with_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
    ) -> Result<PreparedPlan, AutomationError> {
        self.prepare_candidates(action, context, prepared_target, None)
    }

    /// 逐 backend prepare，用户约束在任何能力排序之前过滤候选。
    fn collect(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
        materialized_target: Option<&MaterializedTarget>,
    ) -> Vec<Result<Vec<PreparedCandidate>, PlanRejection>> {
        self.backends
            .iter()
            .filter(|backend| action.target().backend_policy.allows(backend.kind()))
            .map(|backend| {
                backend.prepare_with_materialized_target(
                    action,
                    context,
                    prepared_target,
                    materialized_target,
                )
            })
            .collect()
    }
}

#[async_trait]
impl ActionDispatcher for ActionRouter {
    async fn execute(
        &self,
        action: &AutomationAction,
        scope: AutomationExecutionScope,
    ) -> Result<ActionOutcome, AutomationError> {
        self.execute_with_options(action, scope, ActionExecutionOptions::default())
            .await
    }

    async fn execute_with_options(
        &self,
        action: &AutomationAction,
        scope: AutomationExecutionScope,
        options: ActionExecutionOptions,
    ) -> Result<ActionOutcome, AutomationError> {
        let mut context = self.context_provider.snapshot();
        context.trace_context = options.trace_context.clone();
        if let AutomationExecutionScope::Window {
            handle,
            process_id,
            capabilities,
        } = scope
        {
            context.foreground_window = Some(WindowContext { handle, process_id });
            context.active_process = None;
            // 显式桌面 AppSession 不能意外复用与该应用无关的全局浏览器会话。
            context.browser_session = None;
            // 当前视觉缓存属于宿主捕获的前台画面，不能冒充显式应用窗口缓存。
            context.visual_cache.ready = false;
            if !capabilities.contains(&CapabilityId::WINDOWS_UIA) {
                context.accessibility.ready = false;
            }
        } else if let AutomationExecutionScope::Browser {
            session_id,
            target_id,
        } = scope
        {
            context.foreground_window = None;
            context.active_process = None;
            context.browser_session = Some(crate::BrowserSessionContext {
                session_id,
                target_id,
                attached: true,
            });
            context.accessibility.ready = false;
            context.visual_cache.ready = false;
        }
        let target_wait_deadline = visual_materialization::deadline(action, options.target_wait);
        let prepared_materialization = visual_materialization::prepare(
            self.target_materializer.as_deref(),
            action,
            options.prepared_target.as_ref(),
        )?;
        let mut materialized_target = visual_materialization::materialize(
            prepared_materialization.as_ref(),
            &context,
            options.target_wait,
            target_wait_deadline,
            options.trace_context.as_ref(),
        )
        .await?;
        let postcondition = options.postcondition;
        let mut baseline = match (&postcondition, &self.visual_verification) {
            (Some(PreparedVisualPostcondition::NewText { query }), Some(provider)) => {
                let window = context.foreground_window.as_ref().ok_or_else(|| {
                    AutomationError::BackendUnavailable {
                        backend: BackendKind::SendInput,
                        message: "visual postcondition requires a frozen window context".to_owned(),
                    }
                })?;
                Some(provider.capture_baseline(window, query).await?)
            }
            (Some(PreparedVisualPostcondition::NewText { .. }), None) => {
                return Err(AutomationError::BackendUnavailable {
                    backend: BackendKind::SendInput,
                    message: "visual postcondition provider is not configured".to_owned(),
                });
            }
            (None, _) => None,
        };
        let is_visual_click = materialized_target.is_some();
        let mut outcome = loop {
            let prepared = match self.prepare_candidates(
                action,
                &context,
                options.prepared_target.as_ref(),
                materialized_target.as_ref(),
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    if let (Some(provider), Some(baseline)) =
                        (self.visual_verification.as_ref(), baseline.take())
                    {
                        provider.discard_baseline(baseline).await;
                    }
                    return Err(error);
                }
            };
            let execution_wait = if materialized_target.is_some() {
                TargetWaitPolicy::none()
            } else {
                options.target_wait
            };
            match prepared.execute_with_wait(execution_wait).await {
                Ok(outcome) => break outcome,
                Err(AutomationError::VisualTargetStale { message })
                    if is_visual_click
                        && target_wait_deadline
                            .is_some_and(|deadline| tokio::time::Instant::now() < deadline) =>
                {
                    let refreshed = visual_materialization::materialize(
                        prepared_materialization.as_ref(),
                        &context,
                        options.target_wait,
                        target_wait_deadline,
                        options.trace_context.as_ref(),
                    )
                    .await;
                    match refreshed {
                        Ok(target) => {
                            materialized_target = target;
                        }
                        Err(error) => {
                            if let (Some(provider), Some(baseline)) =
                                (self.visual_verification.as_ref(), baseline.take())
                            {
                                provider.discard_baseline(baseline).await;
                            }
                            return Err(error);
                        }
                    }
                    let _ = message;
                }
                Err(error) => {
                    if let (Some(provider), Some(baseline)) =
                        (self.visual_verification.as_ref(), baseline.take())
                    {
                        provider.discard_baseline(baseline).await;
                    }
                    return Err(error);
                }
            }
        };
        if let (
            Some(PreparedVisualPostcondition::NewText { query }),
            Some(baseline),
            Some(provider),
        ) = (
            postcondition.as_ref(),
            baseline.take(),
            self.visual_verification.as_ref(),
        ) {
            let verification = provider
                .verify_new_text(baseline, &query, options.postcondition_wait)
                .await
                .map_err(|error| match error {
                    AutomationError::OutcomeUnknown { .. } => error,
                    other => AutomationError::OutcomeUnknown {
                        backend: BackendKind::SendInput,
                        message: format!("visual postcondition verification failed: {other}"),
                    },
                })?;
            match verification {
                VisualVerificationResult::Confirmed => {
                    outcome.outputs.insert("confirmed".to_owned(), true.into());
                }
                VisualVerificationResult::Rejected { reason }
                | VisualVerificationResult::Uncertain { reason } => {
                    return Err(AutomationError::OutcomeUnknown {
                        backend: outcome.backend,
                        message: reason,
                    });
                }
            }
        }
        Ok(outcome)
    }
}

impl ActionRouter {
    /// 在同步 API 中准备没有异步视觉物化结果的候选；真实 Visual Click 由异步入口完成物化。
    fn prepare_candidates(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
        materialized_target: Option<&MaterializedTarget>,
    ) -> Result<PreparedPlan, AutomationError> {
        let mut candidates = self
            .collect(action, context, prepared_target, materialized_target)
            .into_iter()
            .filter_map(Result::ok)
            .flatten()
            .filter(|candidate| {
                candidate.explain().support.is_supported()
                    && candidate.explain().availability.is_ready()
            })
            .collect::<Vec<_>>();
        candidates
            .sort_by_key(|candidate| candidate_rank(candidate, &action.target().backend_policy));
        if candidates.is_empty() {
            return Err(AutomationError::NoBackendAvailable);
        }
        Ok(PreparedPlan::new(candidates, self.evidence.clone()))
    }
}

/// 返回真实候选的完整 Planner 排序键；完整分支路径优先于后端能力评分。
fn candidate_rank(
    candidate: &PreparedCandidate,
    policy: &argusflow_core::BackendPolicy,
) -> (argusflow_query::BranchPath, u8, u8, u8, usize, usize) {
    let explain = candidate.explain();
    (
        explain.branch_path.clone().unwrap_or_default(),
        explain.support.rank(),
        explain.context_fitness.rank(),
        explain.cost.rank(),
        policy.preference_rank(explain.backend),
        route_tie_break_rank(explain.backend),
    )
}

/// Explain 沿用完整分支路径、语义支持、availability、上下文、成本和 tie-break 顺序。
fn explain_rank(
    explain: &PlanExplain,
    policy: &argusflow_core::BackendPolicy,
) -> (
    (bool, argusflow_query::BranchPath),
    u8,
    u8,
    u8,
    u8,
    usize,
    usize,
) {
    let branch_rank = explain
        .branch_path
        .clone()
        .map(|path| (false, path))
        .unwrap_or_else(|| (true, argusflow_query::BranchPath::default()));
    (
        branch_rank,
        explain.support.rank(),
        explain.availability.rank(),
        explain.context_fitness.rank(),
        explain.cost.rank(),
        policy.preference_rank(explain.backend),
        route_tie_break_rank(explain.backend),
    )
}

/// 把 compiler/action 拒绝转换为 UI 可展示的 Unsupported explain。
fn rejected_explain(rejection: PlanRejection) -> PlanExplain {
    PlanExplain {
        backend: rejection.backend(),
        branch_path: None,
        support: SupportLevel::Unsupported,
        cost: QueryCost::High,
        availability: RuntimeAvailability::Unavailable,
        context_fitness: ContextFitness::Poor,
        portability: QueryPortability::Portable,
        steps: Vec::new(),
        diagnostics: vec![Diagnostic::global(
            DiagnosticCode::UnsupportedBackend,
            DiagnosticSeverity::Information,
            None,
        )],
    }
}

/// 返回稳定 backend tie-break 序号。
fn route_tie_break_rank(backend: BackendKind) -> usize {
    ROUTE_TIE_BREAK_ORDER
        .iter()
        .position(|candidate| *candidate == backend)
        .unwrap_or(ROUTE_TIE_BREAK_ORDER.len())
}
