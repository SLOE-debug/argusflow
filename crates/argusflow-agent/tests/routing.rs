//! PreparedPlan 排序、ExecutionContext、显式约束与严格 fallback 测试。

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use argusflow_agent::{
    ActionBackend, ActionRouter, BrowserSessionContext, ContextFitness, ExecutionContext,
    PlanExplain, PlanRejection, PreparedCandidate, PreparedExecution, RuntimeAvailability,
    WindowContext,
};
use argusflow_core::{
    ActionOutcome, AqlQuery, AutomationAction, AutomationError, AutomationExecutionScope,
    AutomationTarget, BackendKind, BackendPreference,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 返回固定真实候选的路由测试后端。
struct PlannedBackend {
    /// 后端类别。
    kind: BackendKind,
    /// 语义支持等级。
    support: SupportLevel,
    /// 模拟 backend compiler 冻结的完整 `any` 分支路径。
    branch_path: BranchPath,
    /// 查询成本。
    cost: QueryCost,
    /// 上下文匹配度。
    fitness: ContextFitness,
    /// 是否根据传入 ExecutionContext 动态计算匹配度。
    context_aware: bool,
    /// 执行结果类别。
    result: ExecutionResult,
    /// 验证 fallback 是否触发的计数器。
    executions: Arc<AtomicUsize>,
}

/// 单个 backend 一次返回多条 branch-specific candidate 的测试实现。
struct AlternativeBackend {
    /// 后端类别。
    kind: BackendKind,
    /// 按 compiler 输出顺序保存的分支路径、结果和执行计数器。
    alternatives: Vec<(BranchPath, ExecutionResult, Arc<AtomicUsize>)>,
}

/// 测试 prepared execution 的结果类别。
#[derive(Clone, Copy)]
enum ExecutionResult {
    Success,
    Unavailable,
    TargetNotFound,
}

impl ActionBackend for PlannedBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn prepare(
        &self,
        _action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let explain = PlanExplain {
            backend: self.kind,
            branch_path: Some(self.branch_path.clone()),
            support: self.support,
            cost: self.cost,
            availability: RuntimeAvailability::Ready,
            context_fitness: if self.context_aware {
                context_fitness(self.kind, context)
            } else {
                self.fitness
            },
            portability: QueryPortability::Portable,
            steps: Vec::new(),
            diagnostics: Vec::new(),
        };
        Ok(vec![PreparedCandidate::new(
            explain,
            Arc::new(TestExecution {
                backend: self.kind,
                result: self.result,
                executions: self.executions.clone(),
            }),
        )])
    }
}

impl ActionBackend for AlternativeBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn prepare(
        &self,
        _action: &AutomationAction,
        _context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        Ok(self
            .alternatives
            .iter()
            .map(|(branch_path, result, executions)| {
                let explain = PlanExplain {
                    backend: self.kind,
                    branch_path: Some(branch_path.clone()),
                    support: SupportLevel::Native,
                    cost: QueryCost::Low,
                    availability: RuntimeAvailability::Ready,
                    context_fitness: ContextFitness::Good,
                    portability: QueryPortability::Portable,
                    steps: Vec::new(),
                    diagnostics: Vec::new(),
                };
                PreparedCandidate::new(
                    explain,
                    Arc::new(TestExecution {
                        backend: self.kind,
                        result: *result,
                        executions: executions.clone(),
                    }),
                )
            })
            .collect())
    }
}

/// 已准备且不再接收原始动作的测试执行计划。
#[derive(Debug)]
struct TestExecution {
    /// 结果中的后端类别。
    backend: BackendKind,
    /// 执行结果类别。
    result: ExecutionResult,
    /// 执行次数。
    executions: Arc<AtomicUsize>,
}

impl std::fmt::Debug for ExecutionResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Success => "Success",
            Self::Unavailable => "Unavailable",
            Self::TargetNotFound => "TargetNotFound",
        })
    }
}

#[async_trait]
impl PreparedExecution for TestExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        match self.result {
            ExecutionResult::Success => Ok(ActionOutcome {
                backend: self.backend,
                message: "prepared backend executed".to_owned(),
                outputs: Default::default(),
            }),
            ExecutionResult::Unavailable => Err(AutomationError::BackendUnavailable {
                backend: self.backend,
                message: "test environment unavailable".to_owned(),
            }),
            ExecutionResult::TargetNotFound => Err(AutomationError::TargetNotFound {
                query: "button()".to_owned(),
            }),
        }
    }
}

#[tokio::test]
async fn router_prefers_support_then_context_fitness_then_cost() {
    let router = ActionRouter::new(vec![
        backend(
            BackendKind::WindowsUia,
            SupportLevel::Hybrid,
            QueryCost::Low,
            ContextFitness::Excellent,
            ExecutionResult::Success,
        ),
        backend(
            BackendKind::BrowserCdp,
            SupportLevel::Native,
            QueryCost::Medium,
            ContextFitness::Poor,
            ExecutionResult::Success,
        ),
    ]);

    let outcome = router
        .execute(&portable_click(), AutomationExecutionScope::Current)
        .await
        .expect("prepared plan should execute");
    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
}

#[tokio::test]
async fn router_prefers_earlier_any_branch_before_backend_capability() {
    let router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([1]),
            support: SupportLevel::Native,
            fitness: ContextFitness::Excellent,
            cost: QueryCost::Low,
            ..planned(BackendKind::WindowsUia, ExecutionResult::Success)
        }),
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([0]),
            support: SupportLevel::Hybrid,
            fitness: ContextFitness::Poor,
            cost: QueryCost::High,
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);

    let outcome = router
        .execute(&portable_click(), AutomationExecutionScope::Current)
        .await
        .expect("the backend preserving the earlier fallback branch should execute");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
}

#[tokio::test]
async fn target_not_found_only_advances_to_a_later_any_branch() {
    let same_branch_executions = Arc::new(AtomicUsize::new(0));
    let later_branch_executions = Arc::new(AtomicUsize::new(0));
    let router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([0]),
            result: ExecutionResult::TargetNotFound,
            ..planned(BackendKind::WindowsUia, ExecutionResult::Success)
        }),
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([0]),
            executions: same_branch_executions.clone(),
            ..planned(BackendKind::VisualCache, ExecutionResult::Success)
        }),
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([1]),
            executions: later_branch_executions.clone(),
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);

    let outcome = router
        .execute(&portable_click(), AutomationExecutionScope::Current)
        .await
        .expect("an empty earlier any branch should advance to the next branch");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
    assert_eq!(same_branch_executions.load(Ordering::SeqCst), 0);
    assert_eq!(later_branch_executions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn non_contiguous_backend_alternatives_preserve_global_branch_order() {
    let branch_two_executions = Arc::new(AtomicUsize::new(0));
    let router = ActionRouter::new(vec![
        Arc::new(AlternativeBackend {
            kind: BackendKind::WindowsUia,
            alternatives: vec![
                (
                    BranchPath::from_indices([0]),
                    ExecutionResult::TargetNotFound,
                    Arc::new(AtomicUsize::new(0)),
                ),
                (
                    BranchPath::from_indices([2]),
                    ExecutionResult::Success,
                    branch_two_executions.clone(),
                ),
            ],
        }),
        Arc::new(PlannedBackend {
            branch_path: BranchPath::from_indices([1]),
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);

    let outcome = router
        .execute(&portable_click(), AutomationExecutionScope::Current)
        .await
        .expect("branch one must run before the first backend's branch two");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
    assert_eq!(branch_two_executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn router_honors_backend_constraint_without_mutating_query() {
    let router = ActionRouter::new(vec![
        backend(
            BackendKind::WindowsUia,
            SupportLevel::Native,
            QueryCost::Low,
            ContextFitness::Good,
            ExecutionResult::Success,
        ),
        backend(
            BackendKind::BrowserCdp,
            SupportLevel::Hybrid,
            QueryCost::Medium,
            ContextFitness::Good,
            ExecutionResult::Success,
        ),
    ]);
    let mut action = portable_click();
    let AutomationAction::Click { target } = &mut action else {
        unreachable!("test action is always Click");
    };
    target.backend_preference = BackendPreference::BrowserCdp;

    let outcome = router
        .execute(&action, AutomationExecutionScope::Current)
        .await
        .expect("forced CDP plan should execute");
    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
    assert_eq!(action.target().locator, portable_click().target().locator);
}

#[test]
fn planner_prefers_attached_browser_session_for_equal_semantic_plans() {
    let router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            context_aware: true,
            ..planned(BackendKind::WindowsUia, ExecutionResult::Success)
        }),
        Arc::new(PlannedBackend {
            context_aware: true,
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);
    let context = ExecutionContext {
        foreground_window: Some(WindowContext {
            handle: 1,
            process_id: 10,
        }),
        browser_session: Some(BrowserSessionContext {
            target_id: "page-1".to_owned(),
            attached: true,
        }),
        ..ExecutionContext::default()
    };

    let report = router.inspect(&portable_click(), &context);
    assert_eq!(report.selected_backend, Some(BackendKind::BrowserCdp));

    let desktop_context = ExecutionContext {
        foreground_window: context.foreground_window,
        ..ExecutionContext::default()
    };
    let desktop_report = router.inspect(&portable_click(), &desktop_context);
    assert_eq!(
        desktop_report.selected_backend,
        Some(BackendKind::WindowsUia)
    );
}

#[tokio::test]
async fn unavailable_plan_can_fallback_but_semantic_failure_cannot() {
    let first_executions = Arc::new(AtomicUsize::new(0));
    let fallback_executions = Arc::new(AtomicUsize::new(0));
    let unavailable_router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            executions: first_executions.clone(),
            ..planned(BackendKind::WindowsUia, ExecutionResult::Unavailable)
        }),
        Arc::new(PlannedBackend {
            executions: fallback_executions.clone(),
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);
    let outcome = unavailable_router
        .execute(&portable_click(), AutomationExecutionScope::Current)
        .await
        .expect("environment failure may fallback");
    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
    assert_eq!(first_executions.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_executions.load(Ordering::SeqCst), 1);

    first_executions.store(0, Ordering::SeqCst);
    fallback_executions.store(0, Ordering::SeqCst);
    let semantic_router = ActionRouter::new(vec![
        Arc::new(PlannedBackend {
            executions: first_executions.clone(),
            ..planned(BackendKind::WindowsUia, ExecutionResult::TargetNotFound)
        }),
        Arc::new(PlannedBackend {
            executions: fallback_executions.clone(),
            ..planned(BackendKind::BrowserCdp, ExecutionResult::Success)
        }),
    ]);
    assert!(matches!(
        semantic_router
            .execute(&portable_click(), AutomationExecutionScope::Current)
            .await,
        Err(AutomationError::TargetNotFound { .. })
    ));
    assert_eq!(fallback_executions.load(Ordering::SeqCst), 0);
}

/// 创建默认原生、低成本、Ready 的测试后端。
fn planned(kind: BackendKind, result: ExecutionResult) -> PlannedBackend {
    PlannedBackend {
        kind,
        support: SupportLevel::Native,
        branch_path: BranchPath::root(),
        cost: QueryCost::Low,
        fitness: ContextFitness::Good,
        context_aware: false,
        result,
        executions: Arc::new(AtomicUsize::new(0)),
    }
}

/// 把测试后端包装为 trait object。
fn backend(
    kind: BackendKind,
    support: SupportLevel,
    cost: QueryCost,
    fitness: ContextFitness,
    result: ExecutionResult,
) -> Arc<dyn ActionBackend> {
    Arc::new(PlannedBackend {
        kind,
        support,
        branch_path: BranchPath::root(),
        cost,
        fitness,
        context_aware: false,
        result,
        executions: Arc::new(AtomicUsize::new(0)),
    })
}

/// 将真实执行上下文映射为测试后端的匹配度。
fn context_fitness(kind: BackendKind, context: &ExecutionContext) -> ContextFitness {
    match kind {
        BackendKind::BrowserCdp
            if context
                .browser_session
                .as_ref()
                .is_some_and(|session| session.attached) =>
        {
            ContextFitness::Excellent
        }
        BackendKind::WindowsUia if context.foreground_window.is_some() => ContextFitness::Good,
        _ => ContextFitness::Poor,
    }
}

/// 构造默认自动规划的 portable AQL 点击动作。
fn portable_click() -> AutomationAction {
    AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1("button(name = \"保存\")")),
    }
}
