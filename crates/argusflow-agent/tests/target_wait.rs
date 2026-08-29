//! UI 节点目标等待的共享 deadline、重试分类与冻结计划测试。

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use argusflow_agent::{
    ActionBackend, ActionRouter, ContextFitness, ExecutionContext, PlanExplain, PlanRejection,
    PreparedCandidate, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionCapability, ActionExecutionOptions, ActionOutcome, AqlQuery, AutomationAction,
    AutomationError, AutomationExecutionScope, AutomationTarget, BackendKind, TargetWaitPolicy,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 冻结候选在测试中的确定性单次执行结果。
#[derive(Debug, Clone, Copy)]
enum ProbeResult {
    /// 第一次 materialize 就成功。
    Success,
    /// 达到指定执行次数后成功，之前均返回目标未找到。
    SuccessAfter(usize),
    /// 每次 materialize 都返回目标未找到。
    TargetNotFound,
    /// 第一次执行即返回目标歧义。
    Ambiguous,
    /// 第一次执行即返回动作能力不支持。
    ActionUnsupported,
}

/// 单个冻结 candidate 的分支路径、结果与执行计数。
struct ProbeCandidate {
    /// 模拟 `any(...)` 编译后绑定的完整分支路径。
    branch_path: BranchPath,
    /// 每轮执行返回的确定性结果。
    result: ProbeResult,
    /// 同一 PreparedExecution 被重复调用的次数。
    executions: Arc<AtomicUsize>,
}

/// prepare 次数可观察、能够一次产生多个 fallback candidate 的测试后端。
struct ProbeBackend {
    /// 每次 Router prepare 的调用次数。
    prepares: Arc<AtomicUsize>,
    /// prepare 后冻结的候选定义。
    candidates: Vec<ProbeCandidate>,
}

impl ActionBackend for ProbeBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::WindowsUia
    }

    fn prepare(
        &self,
        _action: &AutomationAction,
        _context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        self.prepares.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .candidates
            .iter()
            .map(|candidate| {
                PreparedCandidate::new(
                    PlanExplain {
                        backend: BackendKind::WindowsUia,
                        branch_path: Some(candidate.branch_path.clone()),
                        support: SupportLevel::Native,
                        cost: QueryCost::Low,
                        availability: RuntimeAvailability::Ready,
                        context_fitness: ContextFitness::Good,
                        portability: QueryPortability::Portable,
                        steps: Vec::new(),
                        diagnostics: Vec::new(),
                    },
                    Arc::new(ProbeExecution {
                        result: candidate.result,
                        executions: candidate.executions.clone(),
                    }),
                )
            })
            .collect())
    }
}

/// 不接收原始 AQL、只重复执行冻结结果的 prepared execution。
#[derive(Debug)]
struct ProbeExecution {
    /// 当前候选的确定性结果。
    result: ProbeResult,
    /// 用于证明等待没有重新 prepare 的执行计数。
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl PreparedExecution for ProbeExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let attempt = self.executions.fetch_add(1, Ordering::SeqCst) + 1;
        match self.result {
            ProbeResult::Success => success_outcome(),
            ProbeResult::SuccessAfter(required_attempts) if attempt >= required_attempts => {
                success_outcome()
            }
            ProbeResult::SuccessAfter(_) | ProbeResult::TargetNotFound => {
                Err(AutomationError::TargetNotFound {
                    query: "button()".to_owned(),
                })
            }
            ProbeResult::Ambiguous => Err(AutomationError::AmbiguousTarget {
                query: "button()".to_owned(),
                matches: 2,
            }),
            ProbeResult::ActionUnsupported => Err(AutomationError::ActionUnsupported {
                backend: BackendKind::WindowsUia,
                query: "button()".to_owned(),
                semantic_matches: 1,
                required: ActionCapability::Activate,
            }),
        }
    }
}

#[tokio::test]
async fn bounded_wait_reuses_one_prepared_plan_until_the_target_appears() {
    let prepares = Arc::new(AtomicUsize::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let router = router(
        prepares.clone(),
        vec![probe(
            BranchPath::root(),
            ProbeResult::SuccessAfter(3),
            executions.clone(),
        )],
    );

    let outcome = execute_with_wait(&router, 500, 10)
        .await
        .expect("the third materialization should observe the target");

    assert_eq!(outcome.backend, BackendKind::WindowsUia);
    assert_eq!(prepares.load(Ordering::SeqCst), 1);
    assert_eq!(executions.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn permanent_miss_returns_the_formal_shared_deadline_error() {
    let executions = Arc::new(AtomicUsize::new(0));
    let router = router(
        Arc::new(AtomicUsize::new(0)),
        vec![probe(
            BranchPath::root(),
            ProbeResult::TargetNotFound,
            executions.clone(),
        )],
    );

    let error = execute_with_wait(&router, 35, 10)
        .await
        .expect_err("a permanent target miss must consume the shared deadline");

    assert!(matches!(
        error,
        AutomationError::TargetWaitTimeout {
            ref query,
            timeout_ms: 35,
        } if query == "button()"
    ));
    assert!(executions.load(Ordering::SeqCst) >= 2);
}

#[tokio::test]
async fn semantic_configuration_failures_are_not_retried() {
    for result in [ProbeResult::Ambiguous, ProbeResult::ActionUnsupported] {
        let executions = Arc::new(AtomicUsize::new(0));
        let router = router(
            Arc::new(AtomicUsize::new(0)),
            vec![probe(BranchPath::root(), result, executions.clone())],
        );

        let error = execute_with_wait(&router, 500, 10)
            .await
            .expect_err("non-recoverable semantic errors must fail immediately");

        assert!(matches!(
            error,
            AutomationError::AmbiguousTarget { .. } | AutomationError::ActionUnsupported { .. }
        ));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn a_ready_later_any_branch_succeeds_in_the_first_attempt() {
    let first_branch = Arc::new(AtomicUsize::new(0));
    let second_branch = Arc::new(AtomicUsize::new(0));
    let router = router(
        Arc::new(AtomicUsize::new(0)),
        vec![
            probe(
                BranchPath::from_indices([0]),
                ProbeResult::TargetNotFound,
                first_branch.clone(),
            ),
            probe(
                BranchPath::from_indices([1]),
                ProbeResult::Success,
                second_branch.clone(),
            ),
        ],
    );

    execute_with_wait(&router, 500, 10)
        .await
        .expect("a ready fallback branch must not wait for the stronger branch");

    assert_eq!(first_branch.load(Ordering::SeqCst), 1);
    assert_eq!(second_branch.load(Ordering::SeqCst), 1);
}

/// 创建仅注册一个确定性 probe backend 的 Router。
fn router(prepares: Arc<AtomicUsize>, candidates: Vec<ProbeCandidate>) -> ActionRouter {
    ActionRouter::new(vec![Arc::new(ProbeBackend {
        prepares,
        candidates,
    })])
}

/// 构造一个冻结 candidate 定义。
fn probe(
    branch_path: BranchPath,
    result: ProbeResult,
    executions: Arc<AtomicUsize>,
) -> ProbeCandidate {
    ProbeCandidate {
        branch_path,
        result,
        executions,
    }
}

/// 通过公开 Dispatcher 边界应用节点级目标等待选项。
async fn execute_with_wait(
    router: &ActionRouter,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> Result<ActionOutcome, AutomationError> {
    router
        .execute_with_options(
            &portable_click(),
            AutomationExecutionScope::Current,
            ActionExecutionOptions {
                target_wait: TargetWaitPolicy::bounded(timeout_ms, poll_interval_ms),
                postcondition_wait: TargetWaitPolicy::default(),
                prepared_target: None,
                postcondition: None,
            },
        )
        .await
}

/// 创建测试候选成功时使用的最小动作结果。
fn success_outcome() -> Result<ActionOutcome, AutomationError> {
    Ok(ActionOutcome {
        backend: BackendKind::WindowsUia,
        message: "prepared backend executed".to_owned(),
        outputs: Default::default(),
        diagnostic_evidence: Vec::new(),
    })
}

/// 构造默认自动规划的 portable AQL 点击动作。
fn portable_click() -> AutomationAction {
    AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1("button(name = \"保存\")")),
    }
}
