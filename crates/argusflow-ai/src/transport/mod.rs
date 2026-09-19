//! 有界兼容 Chat Completions 传输，不保存请求正文或密钥。
mod client;
pub(crate) use client::Client;
pub(crate) trait Transport {
    fn complete(
        &self,
        messages: &[serde_json::Value],
        tools: Vec<serde_json::Value>,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> impl std::future::Future<Output = crate::Result<Response>> + Send;
}
use serde::Deserialize;
use serde_json::Value;
/// 最小服务响应；不完整结束不能作为最终结果。
#[derive(Deserialize)]
pub(crate) struct Response {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Value,
}
#[derive(Deserialize)]
pub(crate) struct Choice {
    pub message: Value,
    pub finish_reason: String,
}
