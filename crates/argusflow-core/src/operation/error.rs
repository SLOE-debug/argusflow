//! 跨能力共享的失败分类，不丢失原始错误来源。

use std::{error::Error, fmt, sync::Arc};

/// 调用方可以穷尽处理的失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// 输入不满足构造或操作约束。
    InvalidInput,
    /// 有界队列或并发额度已满。
    Busy,
    /// 包含排队在内的总截止时间到期。
    Timeout,
    /// 调用方取消了请求。
    Cancelled,
    /// 外部能力或依赖尚不可用。
    Unavailable,
    /// 句柄所属元素、窗口或会话已经失效。
    StaleHandle,
    /// 查询没有结果。
    NotFound,
    /// 唯一查询匹配到多个对象。
    Ambiguous,
    /// 目标不支持所请求的能力。
    Unsupported,
    /// 原生库或操作系统报告失败。
    Native,
    /// 对端消息不满足当前协议契约。
    Protocol,
    /// 输入或处理规模超过资源限制。
    ResourceLimit,
    /// 能力已经关闭。
    Closed,
    /// 取消后底层调用仍未返回，暂时禁止新请求。
    Unresponsive,
}

/// 失败时是否可能已经产生外部副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// 尚未向外部提交副作用操作。
    None,
    /// 操作已提交或进入原生调用，但结果没有得到确认；不可自动重放。
    Unconfirmed,
}

/// 携带阶段、请求关联及原始来源的不可变错误。
#[derive(Debug, Clone)]
pub struct Failure {
    kind: FailureKind,
    stage: &'static str,
    message: String,
    effect: Effect,
    request_id: Option<u64>,
    resource: Option<String>,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl Failure {
    /// 构造尚未产生副作用的错误；message 不应包含图片或输入文字。
    pub fn new(kind: FailureKind, stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            stage,
            message: message.into(),
            effect: Effect::None,
            request_id: None,
            resource: None,
            source: None,
        }
    }

    /// 附加原始错误，仅用于显式诊断。
    pub fn with_source(mut self, source: impl Error + Send + Sync + 'static) -> Self {
        self.source = Some(Arc::new(source));
        self
    }

    /// 返回稳定的错误分类。
    pub fn kind(&self) -> FailureKind {
        self.kind
    }
    /// 返回发生失败的操作阶段。
    pub fn stage(&self) -> &'static str {
        self.stage
    }
    /// 返回不包含用户内容的摘要。
    pub fn message(&self) -> &str {
        &self.message
    }
    /// 返回副作用的不确定性。
    pub fn effect(&self) -> Effect {
        self.effect
    }
    /// 返回关联请求标识；构造阶段失败可能没有标识。
    pub fn request_id(&self) -> Option<u64> {
        self.request_id
    }
    /// 返回失败关联的资源身份，不含输入内容。
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }
    /// 附加能力模块确认的资源身份。
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = Some(resource.into());
        self
    }
    /// 附加当前操作的执行上下文。
    pub fn with_context(mut self, id: u64, effect: Effect) -> Self {
        self.request_id = Some(id);
        self.effect = effect;
        self
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at {}: {} (effect={:?}, request={:?})",
            self.kind, self.stage, self.message, self.effect, self.request_id
        )
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &dyn Error)
    }
}
