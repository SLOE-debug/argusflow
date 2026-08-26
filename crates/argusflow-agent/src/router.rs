use std::sync::Arc;

use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, AutomationExecutionScope, BackendKind,
    BackendPreference, TargetScope,
};
use argusflow_query::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryCost, QueryPortability, SupportLevel,
};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

use crate::{
    ActionBackend, ContextFitness, EvidenceSettings, ExecutionContext, ExecutionContextProvider,
    PlanExplain, PlanRejection, PlanningReport, PreparedCandidate, PreparedPlan,
    RuntimeAvailability, StaticExecutionContext, WindowContext,
};

/// 能力、可用性、上下文和成本相同时使用的稳定兜底顺序。
pub const ROUTE_TIE_BREAK_ORDER: [BackendKind; 7] = [
    BackendKind::WindowsUia,
    BackendKind::BrowserCdp,
    BackendKind::VisualCache,
    BackendKind::OcrTiny,
    BackendKind::OcrMedium,
    BackendKind::GuiGrounding,
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
}

impl ActionRouter {
    /// 使用空上下文创建路由器；宿主可通过 `with_context_provider` 注入真实快照。
    pub fn new(backends: Vec<Arc<dyn ActionBackend>>) -> Self {
        Self {
            backends,
            context_provider: Arc::new(StaticExecutionContext::default()),
            evidence: EvidenceSettings::default(),
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
        }
    }

    /// 为随后生成的 PreparedPlan 注入证据策略与宿主持久化边界。
    pub fn with_evidence(mut self, evidence: EvidenceSettings) -> Self {
        self.evidence = evidence;
        self
    }

    /// 返回稳定 tie-break 顺序，供开发者 Explain 展示。
    pub const fn route_tie_break_order(&self) -> &'static [BackendKind] {
        &ROUTE_TIE_BREAK_ORDER
    }

    /// 使用显式上下文准备并排序所有候选，返回只读 Planner 报告。
    pub fn inspect(&self, action: &AutomationAction, context: &ExecutionContext) -> PlanningReport {
        let mut explains = self
            .collect(action, context)
            .into_iter()
            .flat_map(|result| match result {
                Ok(candidates) => candidates
                    .into_iter()
                    .map(|candidate| candidate.explain().clone())
                    .collect(),
                Err(rejection) => vec![rejected_explain(rejection)],
            })
            .collect::<Vec<_>>();
        explains.sort_by_key(explain_rank);
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
        if matches!(&action.target().scope, TargetScope::Application { .. }) {
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
        let mut candidates = self
            .collect(action, context)
            .into_iter()
            .filter_map(Result::ok)
            .flatten()
            .filter(|candidate| {
                candidate.explain().support.is_supported()
                    && candidate.explain().availability.is_ready()
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(candidate_rank);
        if candidates.is_empty() {
            return Err(AutomationError::NoBackendAvailable);
        }
        Ok(PreparedPlan::new(candidates, self.evidence.clone()))
    }

    /// 逐 backend prepare，用户约束在任何能力排序之前过滤候选。
    fn collect(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Vec<Result<Vec<PreparedCandidate>, PlanRejection>> {
        self.backends
            .iter()
            .filter(|backend| preference_allows(action.target().backend_preference, backend.kind()))
            .map(|backend| backend.prepare(action, context))
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
        let mut context = self.context_provider.snapshot();
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
            if !capabilities.windows_uia {
                context.accessibility.ready = false;
            }
        }
        self.prepare(action, &context)?.execute().await
    }
}

/// 判断显式 backend constraint 是否允许候选参与 Planner。
const fn preference_allows(preference: BackendPreference, backend: BackendKind) -> bool {
    match preference {
        BackendPreference::Auto => true,
        BackendPreference::WindowsUia => matches!(backend, BackendKind::WindowsUia),
        BackendPreference::BrowserCdp => matches!(backend, BackendKind::BrowserCdp),
    }
}

/// 返回真实候选的完整 Planner 排序键；完整分支路径优先于后端能力评分。
fn candidate_rank(
    candidate: &PreparedCandidate,
) -> (argusflow_query::BranchPath, u8, u8, u8, usize) {
    let explain = candidate.explain();
    (
        explain.branch_path.clone().unwrap_or_default(),
        explain.support.rank(),
        explain.context_fitness.rank(),
        explain.cost.rank(),
        route_tie_break_rank(explain.backend),
    )
}

/// Explain 沿用完整分支路径、语义支持、availability、上下文、成本和 tie-break 顺序。
fn explain_rank(
    explain: &PlanExplain,
) -> ((bool, argusflow_query::BranchPath), u8, u8, u8, u8, usize) {
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
