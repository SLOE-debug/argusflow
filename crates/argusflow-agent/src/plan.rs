use std::{fmt, sync::Arc, time::Duration};

use argusflow_core::{
    ActionOutcome, AutomationError, BackendKind, DiagnosticEvidenceReference, TargetWaitMode,
    TargetWaitPolicy,
};
use argusflow_query::{BranchPath, Diagnostic, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;
use serde::Serialize;

use crate::{
    ContextFitness, EvidenceBundle, EvidenceCapturePolicy, EvidenceCaptureRequest, EvidenceOutcome,
    EvidenceRecord, EvidenceSettings, EvidenceTrigger, PreparedDiagnostics,
};

/// 查询语义支持之外的 executor 与运行环境状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAvailability {
    /// Executor 已实现，且所需上下文当前可用。
    Ready,
    /// Executor 已实现，但当前缺少窗口、进程或会话上下文。
    MissingContext,
    /// Executor 已实现，但当前环境明确不可用。
    Unavailable,
    /// 仅有 compiler/plan，executor 尚未接入。
    NotImplemented,
}

impl RuntimeAvailability {
    /// 判断候选能否进入本次实际执行计划。
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    /// 返回从可用到不可用的 explain 排序序号。
    pub const fn rank(self) -> u8 {
        match self {
            Self::Ready => 0,
            Self::MissingContext => 1,
            Self::Unavailable => 2,
            Self::NotImplemented => 3,
        }
    }
}

/// 后端计划步骤类别；产品 UI 可按模式决定展示粒度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepKind {
    /// 搜索范围或关系。
    Scope,
    /// 候选节点来源。
    CandidateSource,
    /// 原生下推条件。
    Pushdown,
    /// 批量缓存或投影属性。
    Cache,
    /// 本地 residual filter。
    Residual,
    /// first/nth 选择规则。
    Selection,
    /// 多分支或额外树遍历。
    Traversal,
    /// 最终动作执行。
    Action,
}

/// Prepared backend plan 中的一项可解释步骤。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanStepExplain {
    /// 步骤类别。
    pub kind: PlanStepKind,
    /// 面向开发者视图的紧凑技术摘要。
    pub summary: String,
}

/// 单个后端候选的真实 compiler 与 runtime explain。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanExplain {
    /// 候选后端。
    pub backend: BackendKind,
    /// 该候选唯一对应的完整 `any(...)` fallback 路径；拒绝候选为 `None`。
    pub branch_path: Option<BranchPath>,
    /// Compiler 对完整语义的支持等级。
    pub support: SupportLevel,
    /// Compiler 计划的预计成本。
    pub cost: QueryCost,
    /// Executor 与当前上下文状态。
    pub availability: RuntimeAvailability,
    /// Backend 对当前上下文的适配度。
    pub context_fitness: ContextFitness,
    /// 查询是否依赖显式 backend 能力。
    pub portability: QueryPortability,
    /// 真实 prepared plan 的步骤摘要。
    pub steps: Vec<PlanStepExplain>,
    /// 语言与 compiler 生成的结构化诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// 所有候选及 Planner 实际选择结果，供 UI 只读展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanningReport {
    /// `None` 表示没有处于 Ready 状态的候选。
    pub selected_backend: Option<BackendKind>,
    /// 已按 Planner 规则排序的全部支持或拒绝候选。
    pub candidates: Vec<PlanExplain>,
}

/// Backend prepare 阶段的结构化拒绝原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanRejection {
    /// Compiler 无法保持动作或查询语义。
    Unsupported {
        /// 拒绝动作的后端。
        backend: BackendKind,
    },
    /// 持久化动作或 AQL 无法解析为有效计划。
    InvalidAction {
        /// 拒绝动作的后端。
        backend: BackendKind,
    },
}

impl PlanRejection {
    /// 返回产生拒绝的后端。
    pub const fn backend(&self) -> BackendKind {
        match self {
            Self::Unsupported { backend } | Self::InvalidAction { backend } => *backend,
        }
    }
}

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
                    error: AutomationError::TargetNotFound { query },
                    candidate_failures,
                } => {
                    if tokio::time::Instant::now() >= deadline {
                        self.capture_final_failures(candidate_failures, true).await;
                        return Err(AutomationError::TargetWaitTimeout {
                            query,
                            timeout_ms: policy.timeout_ms,
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
        AutomationError::NoBackendAvailable
        | AutomationError::BackendFailed { .. }
        | AutomationError::OutcomeUnknown { .. } => None,
    }
}
