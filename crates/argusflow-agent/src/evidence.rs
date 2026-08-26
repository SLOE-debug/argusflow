//! Prepared candidate 失败证据与 backend 采集契约。

use std::{fmt, path::PathBuf, time::Duration};

use argusflow_core::BackendKind;
use argusflow_query::BranchPath;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::PlanExplain;

/// 会触发运行时现场采集的稳定失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTrigger {
    /// 查询完成但没有语义候选。
    TargetNotFound,
    /// 动作适配后仍有多个候选。
    AmbiguousTarget,
    /// 查询存在语义候选，但都不能执行当前动作。
    ActionUnsupported,
    /// 后端或其运行环境当前不可用。
    BackendUnavailable,
    /// 执行或采集超过有界截止时间。
    Timeout,
}

/// PreparedPlan 对候选失败证据的采集时机。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCapturePolicy {
    /// 不采集失败证据。
    #[default]
    Off,
    /// 只有已经确定无法继续回退的最终失败才采集。
    FinalFailure,
    /// 每个候选失败都在回退前采集，包括随后恢复的分支。
    BranchFailure,
}

/// Evidence Bundle 中可独立读取的 artifact 类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceArtifactKind {
    /// 冻结候选的 Planner Explain。
    PlannerExplain,
    /// HWND、PID、frame 或 session 等瞬时执行上下文。
    ExecutionContext,
    /// 查询范围、过滤阶段和 near miss 解释。
    SelectorTrace,
    /// 进程作用域 UIA Control View 快照。
    UiaProcessTree,
    /// UIA 语义候选与逐条件结果。
    UiaCandidateSet,
    /// CDP DOMSnapshot 快照。
    DomSnapshot,
    /// CDP Accessibility Tree 快照。
    AxTree,
    /// 失败现场截图。
    Screenshot,
    /// OCR 推理区域与置信度。
    OcrRegions,
    /// OCR 区域叠加图。
    OcrOverlay,
    /// 受控大小的诊断日志。
    Logs,
}

/// Artifact 的拥有型内容；不得包含后端原生 handle 或 COM interface。
#[derive(Debug, Clone, PartialEq)]
pub enum EvidenceArtifactData {
    /// UTF-8 文本。
    Text(String),
    /// 可稳定序列化的结构化 JSON。
    Json(Value),
    /// PNG 等不透明二进制数据。
    Binary(Vec<u8>),
}

/// 一个 Evidence Bundle 内的具名 artifact。
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceArtifact {
    /// 稳定 artifact 类别。
    pub kind: EvidenceArtifactKind,
    /// 相对于本次 evidence 目录的路径。
    pub relative_path: PathBuf,
    /// 是否可能包含需要谨慎展示和清理的用户数据。
    pub sensitive: bool,
    /// 完全脱离后端线程所有权的内容。
    pub data: EvidenceArtifactData,
}

/// 单个冻结候选失败时采集到的内存证据集合。
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceBundle {
    /// 当前稳定 manifest schema 版本。
    pub schema_version: u16,
    /// 采集证据的后端。
    pub backend: BackendKind,
    /// 候选绑定的完整 AQL fallback 路径。
    pub branch_path: BranchPath,
    /// 触发采集的失败分类。
    pub trigger: EvidenceTrigger,
    /// 规范化查询；不包含运行时读取到的用户值。
    pub query: String,
    /// 可分别持久化和按需读取的 artifact。
    pub artifacts: Vec<EvidenceArtifact>,
}

impl EvidenceBundle {
    /// 创建 schema v1 的空 bundle，backend collector 再追加自己的 artifacts。
    pub fn new(
        backend: BackendKind,
        branch_path: BranchPath,
        trigger: EvidenceTrigger,
        query: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: 1,
            backend,
            branch_path,
            trigger,
            query: query.into(),
            artifacts: Vec::new(),
        }
    }

    /// 追加一个已经脱离 backend 原生线程的 artifact。
    pub fn push(&mut self, artifact: EvidenceArtifact) {
        self.artifacts.push(artifact);
    }
}

/// 单次诊断采集的硬资源预算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceBudget {
    /// 整次采集允许占用的墙钟时间。
    pub deadline: Duration,
    /// backend tree snapshot 最多包含的节点数。
    pub max_nodes: usize,
    /// 层级快照最多展开的深度。
    pub max_depth: usize,
    /// 单个 bundle 允许持久化的最大字节数。
    pub max_bytes: usize,
    /// selector trace 最多保留的 near miss 数量。
    pub max_near_misses: usize,
}

impl Default for EvidenceBudget {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(3),
            max_nodes: 1_000,
            max_depth: 32,
            max_bytes: 16 * 1024 * 1024,
            max_near_misses: 20,
        }
    }
}

/// 敏感证据与磁盘占用的宿主策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceRetentionPolicy {
    /// 是否允许持久化屏幕像素。
    pub persist_screenshot: bool,
    /// 是否允许采集输入控件的值；backend 仍必须屏蔽密码控件。
    pub persist_text_values: bool,
    /// 是否要求 backend 对密码或受保护控件脱敏。
    pub redact_password_controls: bool,
    /// sink 接受的单个 bundle 最大总字节数。
    pub max_total_bytes: usize,
    /// 宿主清理任务可以使用的可选保留期。
    pub ttl: Option<Duration>,
}

impl Default for EvidenceRetentionPolicy {
    fn default() -> Self {
        Self {
            persist_screenshot: false,
            persist_text_values: false,
            redact_password_controls: true,
            max_total_bytes: 16 * 1024 * 1024,
            ttl: Some(Duration::from_secs(7 * 24 * 60 * 60)),
        }
    }
}

/// PreparedPlan 传给冻结 backend diagnostics 的完整采集请求。
#[derive(Debug, Clone)]
pub struct EvidenceCaptureRequest {
    /// 已分类且允许采集的失败触发器。
    pub trigger: EvidenceTrigger,
    /// 当前候选完整、只读的 Planner Explain。
    pub explain: PlanExplain,
    /// 本次采集硬预算。
    pub budget: EvidenceBudget,
    /// 由宿主决定的敏感数据策略。
    pub retention: EvidenceRetentionPolicy,
}

/// Backend collector 失败；该错误永远不能替代原始 AutomationError。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceCaptureError {
    /// 请求与 prepared diagnostics 绑定的后端不一致。
    #[error("evidence request backend does not match prepared diagnostics")]
    BackendMismatch,
    /// 采集超过有界截止时间。
    #[error("evidence capture exceeded its deadline")]
    DeadlineExceeded,
    /// 后端现场已经失效或不可访问。
    #[error("evidence source is unavailable: {message}")]
    SourceUnavailable {
        /// 不包含敏感内容的稳定摘要。
        message: String,
    },
    /// 后端采集失败。
    #[error("evidence capture failed: {message}")]
    CaptureFailed {
        /// 不包含 artifact bytes 的错误摘要。
        message: String,
    },
}

/// prepare 阶段绑定冻结查询、上下文与 backend session 的诊断对象。
#[async_trait]
pub trait PreparedDiagnostics: fmt::Debug + Send + Sync {
    /// 返回负责采集的后端。
    fn backend(&self) -> BackendKind;

    /// 采集普通 Rust DTO；禁止把 COM、CDP session handle 等原生对象返回给调用方。
    async fn capture(
        &self,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError>;
}

/// 分支失败最终在整次 PreparedPlan 中的结局。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvidenceOutcome {
    /// PreparedPlan 最终仍然失败。
    FinalFailure,
    /// 更晚的冻结候选成功执行。
    RecoveredByFallback {
        /// 完成恢复的候选分支。
        recovered_branch: BranchPath,
    },
}

/// 交给 sink 的完整证据记录。
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceRecord {
    /// backend collector 产生的证据。
    pub bundle: EvidenceBundle,
    /// 失败候选在计划中的最终结局。
    pub outcome: EvidenceOutcome,
    /// 持久化时仍需执行的敏感数据和大小约束。
    pub retention: EvidenceRetentionPolicy,
}
