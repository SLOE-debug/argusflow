use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::BackendKind;

/// 自动化后端无法执行动作时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AutomationError {
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
