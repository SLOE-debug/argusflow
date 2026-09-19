//! 对外错误不携带密钥、HTTP 响应正文或完整录制内容。
/// AI 模块结果。
pub type Result<T> = std::result::Result<T, AiError>;
/// 可辨认的故障边界。
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// 配置或证据违反当前契约。
    #[error("{0}")]
    Invalid(String),
    /// 持久化失败。
    #[error("AI 数据库操作失败：{0}")]
    Storage(#[from] rusqlite::Error),
    /// 本机文件读取失败。
    #[error("证据文件读取失败：{0}")]
    Io(#[from] std::io::Error),
    /// 用户取消。
    #[error("AI 整理已取消")]
    Cancelled,
    /// 模型传输失败，不转发可能包含请求 URL/密钥的底层错误。
    #[error("模型请求失败：{0}")]
    Transport(String),
    /// 有界预算耗尽。
    #[error("AI 整理预算耗尽：{0}")]
    Budget(String),
}
impl From<serde_json::Error> for AiError {
    fn from(_: serde_json::Error) -> Self {
        Self::Invalid("JSON 结构不符合当前契约".into())
    }
}
