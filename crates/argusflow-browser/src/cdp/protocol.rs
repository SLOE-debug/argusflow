//! CDP 响应解码和请求关联，JSON 限制在协议边界。
use super::{connection::Request, lifecycle::Health};
use crate::BrowserError as Failure;
use argusflow_core::FailureKind;
use serde_json::Value;
use std::collections::HashMap;

pub(crate) fn receive(
    text: &str,
    pending: &mut HashMap<u64, Request>,
    health: &Health,
) -> Result<(), Failure> {
    let message: Value = serde_json::from_str(text).map_err(|error| {
        Failure::new(FailureKind::Protocol, "cdp_decode", "CDP 消息不是有效 JSON")
            .with_source(error)
    })?;
    if let Some(id) = message.get("id").and_then(Value::as_u64) {
        // 已取消或过期的请求可能仍收到迟到响应，安全丢弃。
        let Some(request) = pending.remove(&id) else {
            return Ok(());
        };
        if message
            .get("sessionId")
            .and_then(Value::as_str)
            .is_some_and(|session| Some(session) != request.session.as_deref())
        {
            let failure = invalid("响应 sessionId 与请求不匹配");
            let _ = request
                .response
                .send(Err(request.operation.contextualize(failure.clone())));
            return Err(failure);
        }
        tracing::debug!(
            request_id = request.operation.id(),
            cdp_id = id,
            method = request.method,
            session = request.session,
            "CDP response received"
        );
        let result = if let Some(error) = message.get("error") {
            let code = error
                .get("code")
                .and_then(Value::as_i64)
                .ok_or_else(|| invalid("CDP 错误没有整数 code"))?;
            let source = CdpError {
                code,
                message: error["message"]
                    .as_str()
                    .unwrap_or("unspecified CDP error")
                    .to_owned(),
                data: error.get("data").cloned(),
            };
            Err(Failure::new(
                if code == -32601 {
                    FailureKind::Unsupported
                } else {
                    FailureKind::Protocol
                },
                request.method,
                format!("CDP 方法被拒绝，code={code}，session={:?}", request.session),
            )
            .with_source(source))
        } else {
            message
                .get("result")
                .cloned()
                .ok_or_else(|| invalid("CDP 响应缺少 result"))
        };
        let result = request
            .operation
            .check("cdp_response")
            .map_err(Failure::from)
            .and(result)
            .map_err(|failure| request.operation.contextualize(failure));
        let _ = request.response.send(result);
    } else if message.get("method").and_then(Value::as_str).is_some() {
        health.event(&message);
    } else {
        return Err(invalid("CDP 消息缺少请求 id 或事件 method"));
    }
    Ok(())
}
pub(crate) fn fail_all(pending: &mut HashMap<u64, Request>, failure: Failure) {
    for (_, request) in pending.drain() {
        let _ = request
            .response
            .send(Err(request.operation.contextualize(failure.clone())));
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailureKind::Protocol, "cdp_decode", message)
}

/// 原始协议诊断仅保留在 source 中，不通过默认 tracing 输出页面内容。
#[derive(Debug, thiserror::Error)]
#[error("CDP {code}: {message}; data={data:?}")]
struct CdpError {
    code: i64,
    message: String,
    data: Option<Value>,
}
