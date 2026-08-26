//! PreparedDiagnostics 采集时机、fallback 结局与错误隔离测试。

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use argusflow_agent::{
    ActionBackend, ActionRouter, ContextFitness, EvidenceBundle, EvidenceCaptureError,
    EvidenceCapturePolicy, EvidenceCaptureRequest, EvidenceOutcome, EvidenceSettings,
    ExecutionContext, InMemoryEvidenceSink, PlanExplain, PlanRejection, PreparedCandidate,
    PreparedDiagnostics, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AqlQuery, AutomationAction, AutomationError, AutomationExecutionScope,
    AutomationTarget, BackendKind,
};
use argusflow_query::{BranchPath, QueryCost, QueryPortability, SupportLevel};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 返回单个冻结 candidate 的证据测试后端。
struct EvidenceBackend {
    /// candidate 后端。
    backend: BackendKind,
    /// AQL fallback 路径。
    branch_path: BranchPath,
    /// 执行结果。
    result: EvidenceExecutionResult,
    /// 可选 diagnostics。
    diagnostics: Option<Arc<dyn PreparedDiagnostics>>,
}

impl ActionBackend for EvidenceBackend {
    fn kind(&self) -> BackendKind {
        self.backend
    }

    fn prepare(
        &self,
        _action: &AutomationAction,
        _context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        let explain = PlanExplain {
            backend: self.backend,
            branch_path: Some(self.branch_path.clone()),
            support: SupportLevel::Native,
            cost: QueryCost::Low,
            availability: RuntimeAvailability::Ready,
            context_fitness: ContextFitness::Good,
            portability: QueryPortability::Portable,
            steps: Vec::new(),
            diagnostics: Vec::new(),
        };
        let candidate = PreparedCandidate::new(
            explain,
            Arc::new(EvidenceExecution {
                backend: self.backend,
                result: self.result,
            }),
        );
        Ok(vec![if let Some(diagnostics) = &self.diagnostics {
            candidate.with_diagnostics(diagnostics.clone())
        } else {
            candidate
        }])
    }
}

/// 证据测试 candidate 的封闭执行结果。
#[derive(Debug, Clone, Copy)]
enum EvidenceExecutionResult {
    /// 动作成功。
    Success,
    /// selector 当前分支为空。
    TargetNotFound,
}

/// 不重新接收原始 action 的测试 execution。
#[derive(Debug)]
struct EvidenceExecution {
    /// 结果后端。
    backend: BackendKind,
    /// 冻结结果。
    result: EvidenceExecutionResult,
}

#[async_trait]
impl PreparedExecution for EvidenceExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        match self.result {
            EvidenceExecutionResult::Success => Ok(ActionOutcome {
                backend: self.backend,
                message: "fallback recovered".to_owned(),
                outputs: Default::default(),
                diagnostic_evidence: Vec::new(),
            }),
            EvidenceExecutionResult::TargetNotFound => Err(AutomationError::TargetNotFound {
                query: "button()".to_owned(),
            }),
        }
    }
}

/// 记录 capture 次数并可注入失败的 prepared diagnostics。
#[derive(Debug)]
struct TestDiagnostics {
    /// 证据后端。
    backend: BackendKind,
    /// capture 调用次数。
    captures: Arc<AtomicUsize>,
    /// 是否让 collector 主动失败。
    fail: bool,
}

#[async_trait]
impl PreparedDiagnostics for TestDiagnostics {
    fn backend(&self) -> BackendKind {
        self.backend
    }

    async fn capture(
        &self,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        self.captures.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(EvidenceCaptureError::CaptureFailed {
                message: "injected evidence failure".to_owned(),
            });
        }
        Ok(EvidenceBundle::new(
            self.backend,
            request.explain.branch_path.unwrap_or_default(),
            request.trigger,
            "button()",
        ))
    }
}

#[tokio::test]
async fn branch_failure_is_persisted_as_recovered_after_fallback() {
    let captures = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(InMemoryEvidenceSink::default());
    let router = ActionRouter::new(vec![
        backend(
            BackendKind::WindowsUia,
            0,
            EvidenceExecutionResult::TargetNotFound,
            Some(Arc::new(TestDiagnostics {
                backend: BackendKind::WindowsUia,
                captures: captures.clone(),
                fail: false,
            })),
        ),
        backend(
            BackendKind::BrowserCdp,
            1,
            EvidenceExecutionResult::Success,
            None,
        ),
    ])
    .with_evidence(EvidenceSettings {
        policy: EvidenceCapturePolicy::BranchFailure,
        sink: sink.clone(),
        ..EvidenceSettings::default()
    });

    let outcome = router
        .execute(&click(), AutomationExecutionScope::Current)
        .await
        .expect("later selector branch should recover");

    assert_eq!(outcome.backend, BackendKind::BrowserCdp);
    assert_eq!(outcome.diagnostic_evidence.len(), 1);
    assert!(outcome.diagnostic_evidence[0].recovered_by_fallback);
    assert_eq!(captures.load(Ordering::SeqCst), 1);
    let records = sink.records();
    assert_eq!(records.len(), 1);
    assert!(matches!(
        records[0].outcome,
        EvidenceOutcome::RecoveredByFallback { .. }
    ));
}

#[tokio::test]
async fn collector_failure_does_not_replace_the_automation_error() {
    let captures = Arc::new(AtomicUsize::new(0));
    let router = ActionRouter::new(vec![backend(
        BackendKind::WindowsUia,
        0,
        EvidenceExecutionResult::TargetNotFound,
        Some(Arc::new(TestDiagnostics {
            backend: BackendKind::WindowsUia,
            captures: captures.clone(),
            fail: true,
        })),
    )])
    .with_evidence(EvidenceSettings {
        policy: EvidenceCapturePolicy::FinalFailure,
        ..EvidenceSettings::default()
    });

    let result = router
        .execute(&click(), AutomationExecutionScope::Current)
        .await;

    assert!(matches!(
        result,
        Err(AutomationError::TargetNotFound { .. })
    ));
    assert_eq!(captures.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn final_failure_preserves_every_branch_snapshot_before_fallback() {
    let captures = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(InMemoryEvidenceSink::default());
    let diagnostics = |backend| {
        Some(Arc::new(TestDiagnostics {
            backend,
            captures: captures.clone(),
            fail: false,
        }) as Arc<dyn PreparedDiagnostics>)
    };
    let router = ActionRouter::new(vec![
        backend(
            BackendKind::WindowsUia,
            0,
            EvidenceExecutionResult::TargetNotFound,
            diagnostics(BackendKind::WindowsUia),
        ),
        backend(
            BackendKind::BrowserCdp,
            1,
            EvidenceExecutionResult::TargetNotFound,
            diagnostics(BackendKind::BrowserCdp),
        ),
    ])
    .with_evidence(EvidenceSettings {
        policy: EvidenceCapturePolicy::FinalFailure,
        sink: sink.clone(),
        ..EvidenceSettings::default()
    });

    let result = router
        .execute(&click(), AutomationExecutionScope::Current)
        .await;

    assert!(matches!(
        result,
        Err(AutomationError::TargetNotFound { .. })
    ));
    assert_eq!(captures.load(Ordering::SeqCst), 2);
    let records = sink.records();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|record| record.outcome == EvidenceOutcome::FinalFailure)
    );
}

/// 创建单候选测试后端。
fn backend(
    backend: BackendKind,
    branch: usize,
    result: EvidenceExecutionResult,
    diagnostics: Option<Arc<dyn PreparedDiagnostics>>,
) -> Arc<dyn ActionBackend> {
    Arc::new(EvidenceBackend {
        backend,
        branch_path: BranchPath::from_indices([branch]),
        result,
        diagnostics,
    })
}

/// 创建包含两个 fallback 分支的 portable action。
fn click() -> AutomationAction {
    AutomationAction::Click {
        target: AutomationTarget::query(AqlQuery::v1(
            "any(button(name = \"missing\"), button(name = \"fallback\"))",
        )),
    }
}
