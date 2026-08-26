//! 持久 CDP WebSocket 的有界请求多路复用。

use std::collections::HashMap;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// CDP 连接、协议或远端方法错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CdpProtocolError {
    /// WebSocket 无法建立或在请求期间断开。
    #[error("CDP transport failed: {message}")]
    Transport {
        /// 不包含页面内容的传输错误摘要。
        message: String,
    },
    /// Chromium 返回结构化协议错误。
    #[error("CDP method {method} failed ({code}): {message}")]
    Method {
        /// 失败的方法名称。
        method: String,
        /// CDP 错误代码。
        code: i64,
        /// CDP 错误消息。
        message: String,
    },
    /// Chromium 响应缺少当前执行器要求的字段。
    #[error("invalid CDP response: {message}")]
    InvalidResponse {
        /// 响应结构错误说明。
        message: String,
    },
}

/// 单条持久 WebSocket 的轻量异步调用句柄。
#[derive(Debug, Clone)]
pub(crate) struct CdpConnection {
    /// actor 的有界请求通道，提供自然背压。
    sender: mpsc::Sender<CdpRequest>,
}

impl CdpConnection {
    /// 建立 WebSocket 并启动唯一读写 actor。
    pub(crate) async fn connect(web_socket_url: &str) -> Result<Self, CdpProtocolError> {
        let (socket, _) =
            connect_async(web_socket_url)
                .await
                .map_err(|error| CdpProtocolError::Transport {
                    message: error.to_string(),
                })?;
        let (sender, receiver) = mpsc::channel(128);
        tokio::spawn(run_connection(socket, receiver));
        Ok(Self { sender })
    }

    /// 在浏览器或指定 target session 上调用一个 CDP 方法。
    pub(crate) async fn command(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpProtocolError> {
        let (response_sender, response_receiver) = oneshot::channel();
        let request = CdpRequest {
            method: method.to_owned(),
            params,
            session_id: session_id.map(str::to_owned),
            response: response_sender,
        };
        self.sender
            .send(request)
            .await
            .map_err(|_| CdpProtocolError::Transport {
                message: "CDP connection actor is unavailable".to_owned(),
            })?;
        response_receiver
            .await
            .map_err(|_| CdpProtocolError::Transport {
                message: "CDP connection closed before returning a response".to_owned(),
            })?
    }
}

/// actor 队列中的冻结方法调用。
struct CdpRequest {
    /// CDP method 名称。
    method: String,
    /// 已序列化参数对象。
    params: Value,
    /// 扁平 session 模式下的 target session ID。
    session_id: Option<String>,
    /// 唯一响应通道。
    response: oneshot::Sender<Result<Value, CdpProtocolError>>,
}

/// 单个尚未收到响应的调用。
struct PendingRequest {
    /// 用于错误归因的方法名。
    method: String,
    /// 原请求的响应通道。
    response: oneshot::Sender<Result<Value, CdpProtocolError>>,
}

/// 在同一任务中拥有 WebSocket 两端和 pending map，避免跨任务锁竞争。
async fn run_connection(
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    mut receiver: mpsc::Receiver<CdpRequest>,
) {
    let (mut writer, mut reader) = socket.split();
    let mut next_id = 1_u64;
    let mut pending = HashMap::<u64, PendingRequest>::new();
    loop {
        tokio::select! {
            request = receiver.recv() => {
                let Some(request) = request else {
                    break;
                };
                let request_id = next_id;
                next_id = next_id.wrapping_add(1).max(1);
                let mut message = json!({
                    "id": request_id,
                    "method": request.method,
                    "params": request.params,
                });
                if let Some(session_id) = &request.session_id {
                    message["sessionId"] = Value::String(session_id.clone());
                }
                let method = message["method"].as_str().unwrap_or_default().to_owned();
                pending.insert(request_id, PendingRequest {
                    method,
                    response: request.response,
                });
                if let Err(error) = writer.send(Message::Text(message.to_string().into())).await {
                    fail_pending(&mut pending, error.to_string());
                    break;
                }
            }
            incoming = reader.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        handle_message(&text, &mut pending);
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        fail_pending(&mut pending, "CDP WebSocket closed".to_owned());
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        fail_pending(&mut pending, error.to_string());
                        break;
                    }
                }
            }
        }
    }
}

/// 解析响应帧；没有 id 的 CDP event 由当前无订阅执行器直接忽略。
fn handle_message(text: &str, pending: &mut HashMap<u64, PendingRequest>) {
    let Ok(message) = serde_json::from_str::<Value>(text) else {
        return;
    };
    let Some(request_id) = message.get("id").and_then(Value::as_u64) else {
        return;
    };
    let Some(request) = pending.remove(&request_id) else {
        return;
    };
    if let Some(error) = message.get("error") {
        let _ = request.response.send(Err(CdpProtocolError::Method {
            method: request.method,
            code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown CDP method error")
                .to_owned(),
        }));
        return;
    }
    let result = message
        .get("result")
        .cloned()
        .ok_or_else(|| CdpProtocolError::InvalidResponse {
            message: format!("{} response did not contain result", request.method),
        });
    let _ = request.response.send(result);
}

/// 连接失败时一次性完成全部 pending 调用，避免调用方永久等待。
fn fail_pending(pending: &mut HashMap<u64, PendingRequest>, message: String) {
    for (_, request) in pending.drain() {
        let _ = request.response.send(Err(CdpProtocolError::Transport {
            message: message.clone(),
        }));
    }
}
