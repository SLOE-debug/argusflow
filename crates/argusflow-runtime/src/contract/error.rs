//! 不吞失首个错误的运行诊断。
use argusflow_core::{Effect, Failure, FailureKind};
use argusflow_workflow::ErrorKind;

/// 运行路径上的精确位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLocation {
    /// 作用域文档 ID。
    pub scope: String,
    /// 作用域激活实例。
    pub instance: u64,
    /// 当前节点 ID；作用域退出时可为空。
    pub node: Option<String>,
    /// 本次运行内从一开始的节点执行序号；重试沿用此序号。
    pub execution: Option<u64>,
}

/// 主错误与后续清理错误；默认不携带用户数据。
#[derive(Debug, Clone)]
pub struct RunError {
    /// 可穷尽匹配的类型。
    pub kind: ErrorKind,
    /// 安全摘要或用户声明的错误代码。
    pub message: String,
    /// 失败是否可能已产生外部副作用。
    pub effect: Effect,
    /// 从根到失败帧的位置。
    pub path: Vec<ExecutionLocation>,
    /// 保留的后续失败，不能覆盖主错误。
    pub secondary: Vec<RunError>,
    /// 仅显式诊断时访问底层来源。
    pub source: Option<Box<Failure>>,
}
impl RunError {
    /// 构造尚无副作用和执行路径的错误。
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            effect: Effect::None,
            path: Vec::new(),
            secondary: Vec::new(),
            source: None,
        }
    }
    /// 标记不确定副作用，禁止自动重放。
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effect = effect;
        self
    }
    pub(crate) fn catchable(&self) -> bool {
        self.kind.catchable() && self.secondary.iter().all(Self::catchable)
    }
}
impl From<Failure> for RunError {
    fn from(error: Failure) -> Self {
        let kind = match error.kind() {
            FailureKind::Timeout => ErrorKind::Timeout,
            FailureKind::Cancelled => ErrorKind::Cancelled,
            FailureKind::Busy => ErrorKind::Busy,
            FailureKind::Unavailable | FailureKind::Closed | FailureKind::Unresponsive => {
                ErrorKind::Unavailable
            }
            FailureKind::NotFound => ErrorKind::NotFound,
            FailureKind::Ambiguous => ErrorKind::Ambiguous,
            FailureKind::StaleHandle => ErrorKind::Stale,
            FailureKind::InvalidInput
            | FailureKind::Unsupported
            | FailureKind::Native
            | FailureKind::Protocol
            | FailureKind::ResourceLimit => ErrorKind::Operation,
        };
        Self {
            kind,
            message: format!("能力调用失败：{}", error.stage()),
            effect: error.effect(),
            path: Vec::new(),
            secondary: Vec::new(),
            source: Some(Box::new(error)),
        }
    }
}
impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e as _)
    }
}
