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
}

/// 自动化后端无法执行动作时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutomationError {
    /// 目标查询执行完成但没有任何元素满足条件。
    #[error("target was not found for query: {query}")]
    TargetNotFound {
        /// 规范化后的查询，便于执行日志与 Inspector 复现。
        query: String,
    },
    /// 查询返回多个元素且没有使用 first/nth 明确选择。
    #[error("query matched {matches} targets and requires an explicit selection: {query}")]
    AmbiguousTarget {
        /// 规范化后的查询。
        query: String,
        /// 后端解析到的候选数量。
        matches: usize,
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
}
