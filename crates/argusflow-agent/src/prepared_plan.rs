use std::{fmt, sync::Arc, time::Duration};

use argusflow_core::{
    ActionOutcome, AutomationError, BackendKind, DiagnosticEvidenceReference, TargetWaitMode,
    TargetWaitPolicy,
};
use argusflow_query::BranchPath;
use async_trait::async_trait;

use crate::plan::PlanExplain;
use crate::{
    EvidenceBundle, EvidenceCapturePolicy, EvidenceCaptureRequest, EvidenceOutcome, EvidenceRecord,
    EvidenceSettings, EvidenceTrigger, PreparedDiagnostics,
};

/// 已绑定动作数据与 backend compiler plan 的执行对象。
#[async_trait]
pub trait PreparedExecution: fmt::Debug + Send + Sync {
    /// 直接执行已准备计划；禁止重新解析或重新规划原始动作。
    async fn execute(&self) -> Result<ActionOutcome, AutomationError>;
}

/// 单个后端的语义、上下文与具体执行计划绑定结果。
#[derive(Clone)]
pub struct PreparedCandidate {
    /// 只读 explain 数据。
    explain: PlanExplain,
    /// 后端已准备的可执行实例。
    execution: Arc<dyn PreparedExecution>,
    /// 与当前冻结候选绑定的可选后端诊断对象。
    diagnostics: Option<Arc<dyn PreparedDiagnostics>>,
}

impl PreparedCandidate {
    /// 创建一个由 backend compiler 完整证明的候选计划。
    pub fn new(explain: PlanExplain, execution: Arc<dyn PreparedExecution>) -> Self {
        Self {
            explain,
            execution,
            diagnostics: None,
        }
    }

    /// 为候选绑定使用同一份冻结计划和执行上下文的诊断对象。
    pub fn with_diagnostics(mut self, diagnostics: Arc<dyn PreparedDiagnostics>) -> Self {
        debug_assert_eq!(self.backend(), diagnostics.backend());
        self.diagnostics = Some(diagnostics);
        self
    }

    /// 返回候选后端。
    pub const fn backend(&self) -> BackendKind {
        self.explain.backend
    }

    /// 返回只读 explain。
    pub const fn explain(&self) -> &PlanExplain {
        &self.explain
    }

    /// 执行绑定的 backend plan。
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        self.execution.execute().await
    }

    /// 在 fallback 改变现场前执行 best-effort 采集。
    async fn capture(
        &self,
        trigger: EvidenceTrigger,
        settings: &EvidenceSettings,
    ) -> Option<EvidenceBundle> {
        let diagnostics = self.diagnostics.as_ref()?;
        if diagnostics.backend() != self.backend() {
            return None;
        }
        let bundle = diagnostics
            .capture(EvidenceCaptureRequest {
                trigger,
                explain: self.explain.clone(),
                budget: settings.budget,
                retention: effective_retention(settings),
            })
            .await
            .ok()?;
        let expected_branch = self
            .explain
            .branch_path
            .as_ref()
            .cloned()
            .unwrap_or_default();
        if bundle.backend != self.backend() || bundle.branch_path != expected_branch {
            return None;
        }
        Some(bundle)
    }
}

impl fmt::Debug for PreparedCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCandidate")
            .field("explain", &self.explain)
            .field("has_diagnostics", &self.diagnostics.is_some())
            .finish_non_exhaustive()
    }
}

/// Router 排序后冻结的一次实际执行计划。
#[derive(Debug)]
pub struct PreparedPlan {
    /// 只有 Ready 候选会进入此列表，第一项是 Planner 选择结果。
    candidates: Vec<PreparedCandidate>,
    /// 不参与候选排序与 fallback 判定的证据配置。
    evidence: EvidenceSettings,
}

/// 同一冻结计划单轮执行的内部结果。
enum PreparedAttempt {
    Success(ActionOutcome),
    Failure {
        error: AutomationError,
        candidate_failures: Vec<(usize, AutomationError)>,
    },
}

impl PreparedPlan {
    /// 从至少一个已排序 Ready 候选创建计划。
    pub(crate) fn new(candidates: Vec<PreparedCandidate>, evidence: EvidenceSettings) -> Self {
        debug_assert!(!candidates.is_empty());
        Self {
            candidates,
            evidence,
        }
    }

    /// 返回本次实际选择的后端。
    pub fn selected_backend(&self) -> BackendKind {
        self.candidates
            .first()
            .expect("prepared plan has a selected candidate")
            .backend()
    }

    /// 返回本次实际计划的 explain，而不是重新分析原始动作。
    pub fn explain(&self) -> &PlanExplain {
        self.candidates
            .first()
            .expect("prepared plan has a selected candidate")
            .explain()
    }

    /// 不启用节点级等待，依次执行冻结候选一次。
    pub async fn execute(self) -> Result<ActionOutcome, AutomationError> {
        match self.execute_once(true).await {
            PreparedAttempt::Success(outcome) => Ok(outcome),
            PreparedAttempt::Failure { error, .. } => Err(error),
        }
    }

    /// 在共享截止时间内重复同一 PreparedPlan，只重试 `TargetNotFound`。
    pub async fn execute_with_wait(
        self,
        policy: TargetWaitPolicy,
    ) -> Result<ActionOutcome, AutomationError> {
        if policy.mode == TargetWaitMode::None {
            return self.execute().await;
        }
        let timeout = Duration::from_millis(policy.timeout_ms);
        let poll_interval = Duration::from_millis(policy.poll_interval_ms.max(1));
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match self.execute_once(false).await {
                PreparedAttempt::Success(outcome) => return Ok(outcome),
                PreparedAttempt::Failure {
                    error: AutomationError::TargetNotFound { query, details },
                    candidate_failures,
                } => {
                    if tokio::time::Instant::now() >= deadline {
                        self.capture_final_failures(candidate_failures, true).await;
                        return Err(AutomationError::TargetWaitTimeout {
                            query,
                            timeout_ms: policy.timeout_ms,
                            details,
                        });
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    tokio::time::sleep(poll_interval.min(remaining)).await;
                }
                PreparedAttempt::Failure {
                    error,
                    candidate_failures,
                } => {
                    self.capture_final_failures(candidate_failures, false).await;
                    return Err(error);
                }
            }
        }
    }

    /// 完整执行一轮冻结候选；等待中的普通 miss 可以关闭 Evidence 采集。
    async fn execute_once(&self, capture_evidence: bool) -> PreparedAttempt {
        let mut fallback_error = None;
        let mut exhausted_branch: Option<BranchPath> = None;
        let mut captured = Vec::new();
        let mut candidate_failures = Vec::new();
        for (candidate_index, candidate) in self.candidates.iter().enumerate() {
            let branch_path = candidate.explain().branch_path.clone().unwrap_or_default();
            if exhausted_branch
                .as_ref()
                .is_some_and(|exhausted| branch_path.as_slice() <= exhausted.as_slice())
            {
                continue;
            }
            match candidate.execute().await {
                Ok(mut outcome) => {
                    if capture_evidence
                        && self.evidence.policy == EvidenceCapturePolicy::BranchFailure
                    {
                        let evidence = self
                            .persist_captured(
                                captured,
                                EvidenceOutcome::RecoveredByFallback {
                                    recovered_branch: branch_path,
                                },
                            )
                            .await;
                        outcome.diagnostic_evidence.extend(evidence);
                    }
                    return PreparedAttempt::Success(outcome);
                }
                Err(error @ AutomationError::BackendUnavailable { .. }) => {
                    if capture_evidence {
                        self.capture_if_configured(candidate, &error, &mut captured)
                            .await;
                    }
                    candidate_failures.push((candidate_index, error.clone()));
                    fallback_error = Some(error)
                }
                Err(error @ AutomationError::TargetNotFound { .. }) => {
                    exhausted_branch = Some(branch_path);
                    if capture_evidence {
                        self.capture_if_configured(candidate, &error, &mut captured)
                            .await;
                    }
                    candidate_failures.push((candidate_index, error.clone()));
                    fallback_error = Some(error);
                }
                Err(error) => {
                    if capture_evidence {
                        self.capture_if_configured(candidate, &error, &mut captured)
                            .await;
                        let _ = self
                            .persist_captured(captured, EvidenceOutcome::FinalFailure)
                            .await;
                    }
                    candidate_failures.push((candidate_index, error.clone()));
                    return PreparedAttempt::Failure {
                        error,
                        candidate_failures,
                    };
                }
            }
        }
        if capture_evidence {
            let _ = self
                .persist_captured(captured, EvidenceOutcome::FinalFailure)
                .await;
        }
        PreparedAttempt::Failure {
            error: fallback_error.unwrap_or(AutomationError::NoBackendAvailable),
            candidate_failures,
        }
    }

    /// 在等待已经确定结束后采集一次最终现场，避免正常加载阶段反复 dump。
    async fn capture_final_failures(
        &self,
        candidate_failures: Vec<(usize, AutomationError)>,
        target_wait_timed_out: bool,
    ) {
        if self.evidence.policy == EvidenceCapturePolicy::Off {
            return;
        }
        let mut captured = Vec::new();
        for (candidate_index, error) in candidate_failures {
            let Some(candidate) = self.candidates.get(candidate_index) else {
                continue;
            };
            let trigger = if target_wait_timed_out
                && matches!(error, AutomationError::TargetNotFound { .. })
            {
                Some(EvidenceTrigger::Timeout)
            } else {
                evidence_trigger(&error)
            };
            let Some(trigger) = trigger else {
                continue;
            };
            if let Some(bundle) = candidate.capture(trigger, &self.evidence).await {
                captured.push(bundle);
            }
        }
        let _ = self
            .persist_captured(captured, EvidenceOutcome::FinalFailure)
            .await;
    }

    /// 根据策略在 fallback 前暂存现场；是否落盘由整份计划的最终结局决定。
    async fn capture_if_configured(
        &self,
        candidate: &PreparedCandidate,
        error: &AutomationError,
        captured: &mut Vec<EvidenceBundle>,
    ) {
        if self.evidence.policy == EvidenceCapturePolicy::Off {
            return;
        }
        let Some(trigger) = evidence_trigger(error) else {
            return;
        };
        if let Some(bundle) = candidate.capture(trigger, &self.evidence).await {
            captured.push(bundle);
        }
    }

    /// 证据持久化同样是 best-effort，不得改变业务动作结果。
    async fn persist_captured(
        &self,
        captured: Vec<EvidenceBundle>,
        outcome: EvidenceOutcome,
    ) -> Vec<DiagnosticEvidenceReference> {
        let mut references = Vec::new();
        for bundle in captured {
            let backend = bundle.backend;
            let branch_path = bundle.branch_path.as_slice().to_vec();
            let recovered_by_fallback =
                matches!(&outcome, EvidenceOutcome::RecoveredByFallback { .. });
            if let Ok(reference) = self
                .evidence
                .sink
                .persist(EvidenceRecord {
                    bundle,
                    outcome: outcome.clone(),
                    retention: effective_retention(&self.evidence),
                })
                .await
            {
                references.push(DiagnosticEvidenceReference {
                    evidence_id: reference.evidence_id,
                    backend,
                    branch_path,
                    recovered_by_fallback,
                });
            }
        }
        references
    }
}

/// 同时应用 capture budget 与宿主持久化上限。
fn effective_retention(settings: &EvidenceSettings) -> crate::EvidenceRetentionPolicy {
    crate::EvidenceRetentionPolicy {
        max_total_bytes: settings
            .retention
            .max_total_bytes
            .min(settings.budget.max_bytes),
        ..settings.retention
    }
}

/// 只允许具备现场诊断价值的公共错误触发采集。
fn evidence_trigger(error: &AutomationError) -> Option<EvidenceTrigger> {
    match error {
        AutomationError::TargetNotFound { .. } => Some(EvidenceTrigger::TargetNotFound),
        AutomationError::TargetWaitTimeout { .. } => Some(EvidenceTrigger::Timeout),
        AutomationError::AmbiguousTarget { .. } => Some(EvidenceTrigger::AmbiguousTarget),
        AutomationError::ActionUnsupported { .. } => Some(EvidenceTrigger::ActionUnsupported),
        AutomationError::BackendUnavailable { .. } => Some(EvidenceTrigger::BackendUnavailable),
        AutomationError::VisualTargetStale { .. } => None,
        AutomationError::NoBackendAvailable
        | AutomationError::BackendFailed { .. }
        | AutomationError::OutcomeUnknown { .. } => None,
    }
}
