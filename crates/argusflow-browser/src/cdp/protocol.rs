//! 持久 CDP WebSocket 的有界请求多路复用。

use std::{collections::HashMap, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::{failure::CdpProtocolError, lifecycle::CdpConnectionHealth};

/// 单条持久 WebSocket 的轻量异步调用句柄。
#[derive(Debug, Clone)]
pub(crate) struct CdpConnection {
    /// actor 的有界请求通道，提供自然背压。
    sender: mpsc::Sender<CdpRequest>,
    /// command 预检与 actor 事件分发共享的生命周期状态。
    health: Arc<CdpConnectionHealth>,
}

impl CdpConnection {
    /// 建立 WebSocket 并启动唯一读写 actor。
    pub(crate) async fn connect(web_socket_url: &str) -> Result<Self, CdpProtocolError> {
        let (socket, _) = connect_async(web_socket_url).await.map_err(|error| {
            CdpProtocolError::TransportUnavailable {
                message: error.to_string(),
            }
        })?;
        let (sender, receiver) = mpsc::channel(128);
        let health = Arc::new(CdpConnectionHealth::default());
        tokio::spawn(run_connection(socket, receiver, health.clone()));
        Ok(Self { sender, health })
    }

    /// 注册扁平 session 与 target 的绑定，供事件和方法错误归因。
    pub(crate) fn register_session(&self, session_id: String, target_id: String) {
        self.health.register_session(session_id, target_id);
    }

    /// 在浏览器或指定 target session 上调用一个 CDP 方法。
    pub(crate) async fn command(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpProtocolError> {
        if let Some(error) = self.health.unavailable_error(session_id) {
            return Err(error);
        }
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
            .map_err(|_| CdpProtocolError::TransportUnavailable {
                message: "CDP connection actor is unavailable".to_owned(),
            })?;
        response_receiver
            .await
            .map_err(|_| CdpProtocolError::TransportUnavailable {
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
    /// 用于 session/target 生命周期事件选择性失败 pending 请求。
    session_id: Option<String>,
    /// 原请求的响应通道。
    response: oneshot::Sender<Result<Value, CdpProtocolError>>,
}

/// 在同一任务中拥有 WebSocket 两端和 pending map，避免跨任务锁竞争。
async fn run_connection(
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    mut receiver: mpsc::Receiver<CdpRequest>,
    health: Arc<CdpConnectionHealth>,
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
                if let Some(error) = health.unavailable_error(request.session_id.as_deref()) {
                    let _ = request.response.send(Err(error));
                    continue;
                }
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
                    session_id: request.session_id,
                    response: request.response,
                });
                if let Err(error) = writer.send(Message::Text(message.to_string().into())).await {
                    let error = health.mark_transport_unavailable(error.to_string());
                    fail_pending(&mut pending, error);
                    break;
                }
            }
            incoming = reader.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        handle_message(&text, &mut pending, &health);
                    }
                    Some(Ok(Message::Binary(_))) => {
                        fail_pending(
                            &mut pending,
                            CdpProtocolError::InvalidResponse {
                                message: "CDP sent an unexpected binary frame".to_owned(),
                            },
                        );
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        let error = health
                            .mark_transport_unavailable("CDP WebSocket closed".to_owned());
                        fail_pending(&mut pending, error);
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        let error = health.mark_transport_unavailable(error.to_string());
                        fail_pending(&mut pending, error);
                        break;
                    }
                }
            }
        }
    }
}

/// 解析响应帧，并把生命周期事件同步到 pending 与后续 command 预检。
fn handle_message(
    text: &str,
    pending: &mut HashMap<u64, PendingRequest>,
    health: &CdpConnectionHealth,
) {
    let message = match serde_json::from_str::<Value>(text) {
        Ok(message) => message,
        Err(error) => {
            fail_pending(
                pending,
                CdpProtocolError::InvalidResponse {
                    message: format!("CDP text frame was not valid JSON: {error}"),
                },
            );
            return;
        }
    };
    let Some(request_id) = message.get("id").and_then(Value::as_u64) else {
        if message.get("method").is_some() {
            if health.observe_event(&message).is_some() {
                fail_unavailable_pending(pending, health);
            }
        } else {
            fail_pending(
                pending,
                CdpProtocolError::InvalidResponse {
                    message: "CDP message contained neither a request id nor an event method"
                        .to_owned(),
                },
            );
        }
        return;
    };
    let Some(request) = pending.remove(&request_id) else {
        return;
    };
    if let Some(error) = message.get("error") {
        let target_id = health.target_id(request.session_id.as_deref());
        let error = CdpProtocolError::classify_method_rejection(
            request.method,
            error.get("code").and_then(Value::as_i64).unwrap_or(-1),
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown CDP method error")
                .to_owned(),
            request.session_id.as_deref(),
            target_id.as_deref(),
        );
        health.record_unavailable(error.clone());
        let unavailable = error.is_backend_unavailable();
        let _ = request.response.send(Err(error));
        if unavailable {
            fail_unavailable_pending(pending, health);
        }
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

/// 生命周期事件后立即完成所有已受影响的 pending 调用。
fn fail_unavailable_pending(
    pending: &mut HashMap<u64, PendingRequest>,
    health: &CdpConnectionHealth,
) {
    let failed_ids = pending
        .iter()
        .filter_map(|(request_id, request)| {
            health
                .unavailable_error(request.session_id.as_deref())
                .map(|error| (*request_id, error))
        })
        .collect::<Vec<_>>();
    for (request_id, error) in failed_ids {
        if let Some(request) = pending.remove(&request_id) {
            let _ = request.response.send(Err(error));
        }
    }
}

/// 连接失败时一次性完成全部 pending 调用，避免调用方永久等待。
fn fail_pending(pending: &mut HashMap<u64, PendingRequest>, error: CdpProtocolError) {
    for (_, request) in pending.drain() {
        let _ = request.response.send(Err(error.clone()));
    }
}
