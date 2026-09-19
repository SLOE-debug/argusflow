use super::Response;
use crate::{AiConfig, AiError, Result};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
pub(crate) struct Client {
    http: reqwest::Client,
    config: AiConfig,
    key: String,
}
impl Client {
    pub fn new(config: AiConfig, key: String) -> Result<Self> {
        config.validate()?;
        if key.is_empty() {
            return Err(AiError::Invalid("请先在 AI 配置中保存 API Key".into()));
        }
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AiError::Transport("无法创建 HTTP 客户端".into()))?;
        Ok(Self { http, config, key })
    }
}
impl super::Transport for Client {
    async fn complete(
        &self,
        messages: &[Value],
        tools: Vec<Value>,
        cancel: &CancellationToken,
    ) -> Result<Response> {
        let mut body = json!({"model":self.config.model,"temperature":0,"max_tokens":self.config.max_tokens,"messages":messages});
        if self.config.bailian {
            body["enable_thinking"] = json!(false);
        }
        if tools.is_empty() {
            body["response_format"] = json!({"type":"json_object"});
        } else {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        let encoded = serde_json::to_vec(&body)?;
        if encoded.len() > 8 * 1024 * 1024 {
            return Err(AiError::Budget("请求超过 8 MiB".into()));
        }
        let request = async {
            let mut response = self
                .http
                .post(&self.config.endpoint)
                .bearer_auth(&self.key)
                .header("Content-Type", "application/json")
                .body(encoded)
                .send()
                .await
                .map_err(|_| AiError::Transport("网络异常或请求超时".into()))?;
            if !response.status().is_success() {
                return Err(AiError::Transport(format!(
                    "HTTP {}，请检查服务地址、模型及密钥",
                    response.status().as_u16()
                )));
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| AiError::Transport("响应读取失败".into()))?
            {
                if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                    return Err(AiError::Budget("模型响应超过 2 MiB".into()));
                }
                bytes.extend_from_slice(&chunk);
            }
            // 不接受供应商偶然回显的密钥进入分析结果。
            let text = String::from_utf8(bytes)
                .map_err(|_| AiError::Transport("响应编码无效".into()))?
                .replace(&self.key, "[REDACTED]");
            let response: Response = serde_json::from_str(&text)?;
            if response.choices.len() != 1
                || !matches!(
                    response.choices[0].finish_reason.as_str(),
                    "stop" | "tool_calls"
                )
            {
                return Err(AiError::Transport("模型输出未完整结束".into()));
            }
            Ok(response)
        };
        tokio::select! {biased; _=cancel.cancelled()=>Err(AiError::Cancelled), result=request=>result}
    }
}
