use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::BackendKind;

/// 目标实例必须提供的跨后端动作能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCapability {
    /// 可通过 backend 原生语义激活目标。
    Activate,
    /// 可写入且不是只读的值接口。
    WriteValue,
    /// 可读取 Accessible Name 或等价语义文本。
    ReadText,
    /// 可读取 backend 的值接口。
    ReadValue,
    /// 可批量读取链接标题与绝对 URL。
    ReadLinks,
}

/// 自动化后端无法执行动作时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutomationError {
    /// 目标查询执行完成但没有任何元素满足条件。
    #[error("target was not found for query: {query}{details}")]
    TargetNotFound {
        /// 规范化后的查询，便于执行日志与 Inspector 复现。
        query: String,
        /// 后端提供的最后一次查询诊断；不需要额外诊断时为空字符串。
        details: String,
    },
    /// 查询所需的可见窗口没有全部完成捕获，不能安全断言不存在或唯一。
    #[error("OCR observation is incomplete for query: {query}{details}")]
    ObservationIncomplete {
        /// 无法完成严格判定的 AQL。
        query: String,
        /// 缺失窗口或捕获失败的安全摘要。
        details: String,
    },
    /// 节点允许的整体目标等待预算耗尽，最后一次完整计划仍未命中目标。
    #[error("在 {timeout_ms}ms 内未等到目标：{query}{details}")]
    TargetWaitTimeout {
        /// 最后一次单次 materialize 使用的规范化查询。
        query: String,
        /// UI 节点配置的共享总等待预算。
        timeout_ms: u64,
        /// 最后一轮目标查询诊断；截止时间发生在查询内部时可能为空字符串。
        details: String,
    },
    /// 视觉目标在物理输入提交前已经不再对应最初观察到的画面。
    #[error("visual target became stale before input commit: {message}")]
    VisualTargetStale {
        /// 目标、窗口或拓扑复验失败的安全摘要。
        message: String,
    },
    /// 查询返回多个元素且没有使用 first/nth 明确选择。
    #[error("query matched {matches} targets and requires an explicit selection: {query}{details}")]
    AmbiguousTarget {
        /// 规范化后的查询。
        query: String,
        /// 后端解析到的候选数量。
        matches: usize,
        /// 后端提供的可展示候选诊断；不需要额外诊断时为空字符串。
        details: String,
    },
    /// 查询存在语义候选，但没有候选具备当前动作要求的能力。
    #[error(
        "query matched {semantic_matches} semantic targets, but none support {required:?}: {query}"
    )]
    ActionUnsupported {
        /// 解析候选的后端。
        backend: BackendKind,
        /// 规范化后的查询。
        query: String,
        /// action suitability 之前的语义候选数量。
        semantic_matches: usize,
        /// 当前动作要求的稳定能力。
        required: ActionCapability,
    },
    /// 候选后端支持当前动作，但其实现或运行环境暂不可用。
    #[error("backend {backend:?} is unavailable: {message}")]
    BackendUnavailable {
        /// 无法使用的后端。
        backend: BackendKind,
        /// 后端不可用的具体原因。
        message: String,
    },
    /// 路由表中没有任何后端声明可以处理当前动作。
    #[error("no backend can execute this action")]
    NoBackendAvailable,
    /// 后端已尝试执行动作，但执行过程失败且不应继续回退。
    #[error("backend {backend:?} failed: {message}")]
    BackendFailed {
        /// 执行失败的后端。
        backend: BackendKind,
        /// 后端返回的失败原因。
        message: String,
    },
    /// 非幂等动作已经执行，但后置条件无法确认其最终事实，禁止自动重试。
    #[error("backend {backend:?} outcome is unknown: {message}")]
    OutcomeUnknown {
        /// 实际完成物理动作的后端。
        backend: BackendKind,
        /// 未确认的原因。
        message: String,
    },
}
