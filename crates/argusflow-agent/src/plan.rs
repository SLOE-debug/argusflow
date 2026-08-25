use std::{fmt, sync::Arc};

use argusflow_core::{ActionOutcome, AutomationError, BackendKind};
use argusflow_query::{Diagnostic, QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;
use serde::Serialize;

use crate::ContextFitness;

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
}

impl PreparedCandidate {
    /// 创建一个由 backend compiler 完整证明的候选计划。
    pub fn new(explain: PlanExplain, execution: Arc<dyn PreparedExecution>) -> Self {
        Self { explain, execution }
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
}

impl fmt::Debug for PreparedCandidate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCandidate")
            .field("explain", &self.explain)
            .finish_non_exhaustive()
    }
}

/// Router 排序后冻结的一次实际执行计划。
#[derive(Debug)]
pub struct PreparedPlan {
    /// 只有 Ready 候选会进入此列表，第一项是 Planner 选择结果。
    candidates: Vec<PreparedCandidate>,
}

impl PreparedPlan {
    /// 从至少一个已排序 Ready 候选创建计划。
    pub(crate) fn new(candidates: Vec<PreparedCandidate>) -> Self {
        debug_assert!(!candidates.is_empty());
        Self { candidates }
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

    /// 依次执行冻结候选；只有运行环境不可用允许 fallback。
    pub async fn execute(self) -> Result<ActionOutcome, AutomationError> {
        let mut unavailable = None;
        for candidate in self.candidates {
            match candidate.execute().await {
                Ok(outcome) => return Ok(outcome),
                Err(error @ AutomationError::BackendUnavailable { .. }) => {
                    unavailable = Some(error)
                }
                Err(error) => return Err(error),
            }
        }
        Err(unavailable.unwrap_or(AutomationError::NoBackendAvailable))
    }
}
