//! Rust 侧 Windows Named Pipe OCR client。

use std::{sync::RwLock, time::Duration};

use async_trait::async_trait;
use tokio::{
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::Mutex,
};

use crate::{
    error::VisionError,
    ocr::{OcrEngine, OcrRequest, OcrResponse},
};

use super::{
    client::{read_framed_message, write_framed_message},
    protocol::{
        VISION_PROTOCOL_VERSION, WorkerCommand, WorkerHealth, WorkerOcrRequest,
        WorkerProtocolEnvelope, WorkerResponse,
    },
};

/// 通过随机 session token 连接本地 PaddleOCR worker。
#[derive(Debug)]
pub struct NamedPipeOcrEngine {
    /// worker Named Pipe 名称。
    pipe_name: String,
    /// 应用启动时生成并由 worker 校验的会话 token。
    session_token: String,
    /// 单连接串行化 request/response，避免响应错配。
    connection: Mutex<Option<NamedPipeClient>>,
    /// 最近一次 handshake 或请求得到的 health。
    health: RwLock<WorkerHealth>,
}

impl NamedPipeOcrEngine {
    /// 创建尚未连接的 Named Pipe engine。
    pub fn new(pipe_name: impl Into<String>, session_token: impl Into<String>) -> Self {
        Self {
            pipe_name: pipe_name.into(),
            session_token: session_token.into(),
            connection: Mutex::new(None),
            health: RwLock::new(WorkerHealth::starting()),
        }
    }

    /// 返回不含像素内容的 worker 管道名称。
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    /// 主动完成 health handshake，供宿主在 Planner 装配前调用。
    pub async fn refresh_health(&self) -> Result<WorkerHealth, VisionError> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let envelope = WorkerProtocolEnvelope {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            request_id,
            session_token: self.session_token.clone(),
            payload: WorkerCommand::Health,
        };
        let (response, body) = self
            .round_trip(envelope, &[], Duration::from_secs(3))
            .await?;
        if !body.is_empty() {
            return Err(VisionError::Protocol {
                message: "worker health response unexpectedly contained a binary body".to_owned(),
            });
        }
        match response.payload {
            WorkerResponse::Health { health } => Ok(health),
            WorkerResponse::Recognize { .. } => Err(VisionError::Protocol {
                message: "worker returned OCR response for health request".to_owned(),
            }),
        }
    }

    /// 返回最近一次 worker health。
    pub fn health(&self) -> WorkerHealth {
        self.health
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// 记录连接级故障并让 Planner 明确看到 worker 已不可用。
    fn mark_failed(&self, message: impl Into<String>) {
        mark_health_failed(&self.health, message);
    }

    /// 通过一条连接完成完整 framed request/response 交换。
    async fn round_trip(
        &self,
        envelope: WorkerProtocolEnvelope<WorkerCommand>,
        body: &[u8],
        timeout: Duration,
    ) -> Result<(WorkerProtocolEnvelope<WorkerResponse>, Vec<u8>), VisionError> {
        let mut connection = self.connection.lock().await;
        if connection.is_none() {
            let client = ClientOptions::new()
                .open(&self.pipe_name)
                .map_err(|error| {
                    let message = format!("failed to open vision worker pipe: {error}");
                    self.mark_failed(message.clone());
                    VisionError::WorkerUnavailable { message }
                })?;
            *connection = Some(client);
        }
        let mut lease = ConnectionLease {
            connection: &mut *connection,
            health: &self.health,
            keep_connection: false,
            failure_marked: false,
        };
        let client = lease
            .connection
            .as_mut()
            .ok_or_else(|| VisionError::WorkerUnavailable {
                message: "vision worker pipe connection was not installed".to_owned(),
            })?;
        let exchange = async {
            write_framed_message(client, &envelope, body).await?;
            read_framed_message(client).await
        };
        let (response, response_body): (WorkerProtocolEnvelope<WorkerResponse>, Vec<u8>) =
            match tokio::time::timeout(timeout, exchange).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    lease.fail(error.to_string());
                    return Err(error);
                }
                Err(_) => {
                    let error = VisionError::FrameTimeout {
                        timeout_ms: timeout.as_millis() as u64,
                    };
                    lease.fail(error.to_string());
                    return Err(error);
                }
            };
        if response.protocol_version != VISION_PROTOCOL_VERSION
            || response.session_token != self.session_token
        {
            let error = VisionError::Protocol {
                message: "worker protocol version or session token mismatch".to_owned(),
            };
            lease.fail(error.to_string());
            return Err(error);
        }
        if response.request_id != envelope.request_id {
            let error = VisionError::Protocol {
                message: "worker response request_id does not match the active envelope".to_owned(),
            };
            lease.fail(error.to_string());
            return Err(error);
        }
        if !response_body.is_empty() {
            let error = VisionError::Protocol {
                message: "worker response must not contain a binary body".to_owned(),
            };
            lease.fail(error.to_string());
            return Err(error);
        }
        if let WorkerResponse::Recognize {
            error: Some(worker_error),
            ..
        } = &response.payload
            && worker_error.code == "deadline_exceeded"
        {
            // 超时后的同步 Paddle 推理进程会被 worker 主动终止；在返回 FrameTimeout 前先
            // 清除连接和 health，避免下一次 Planner 误以为旧 worker 仍可用。
            lease.fail("worker terminated after OCR deadline exceeded");
        }
        if let WorkerResponse::Health { ref health } = response.payload {
            let mut current = self
                .health
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *current = health.clone();
        }
        lease.keep_connection = !lease.failure_marked;
        Ok((response, response_body))
    }
}

/// 在连接级失败或异步调用被取消时同步更新 worker health。
fn mark_health_failed(health: &RwLock<WorkerHealth>, message: impl Into<String>) {
    let mut health = health
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *health = WorkerHealth::failed(message);
}

/// 保证 `round_trip` 被外层 deadline 取消时不会留下半帧的可复用 Named Pipe 连接。
struct ConnectionLease<'a> {
    /// 当前被该请求独占的连接槽。
    connection: &'a mut Option<NamedPipeClient>,
    /// 与连接槽共享的健康状态。
    health: &'a RwLock<WorkerHealth>,
    /// 只有完整校验成功的响应才能保留连接。
    keep_connection: bool,
    /// 已显式记录失败时避免 Drop 覆盖原始错误原因。
    failure_marked: bool,
}

impl ConnectionLease<'_> {
    /// 清除当前连接并把失败原因发布给 Planner。
    fn fail(&mut self, message: impl Into<String>) {
        *self.connection = None;
        mark_health_failed(self.health, message);
        self.failure_marked = true;
    }
}

impl Drop for ConnectionLease<'_> {
    fn drop(&mut self) {
        if self.keep_connection || self.failure_marked {
            return;
        }
        *self.connection = None;
        mark_health_failed(
            self.health,
            "vision worker request was cancelled before response validation",
        );
    }
}

#[async_trait]
impl OcrEngine for NamedPipeOcrEngine {
    fn health(&self) -> WorkerHealth {
        NamedPipeOcrEngine::health(self)
    }

    async fn recognize(&self, request: OcrRequest) -> Result<OcrResponse, VisionError> {
        let (wire_request, body) = WorkerOcrRequest::from_request(&request)?;
        let envelope = WorkerProtocolEnvelope {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            request_id: wire_request.request_id.clone(),
            session_token: self.session_token.clone(),
            payload: WorkerCommand::Recognize {
                request: wire_request,
            },
        };
        let (response, response_body) = self.round_trip(envelope, &body, request.deadline).await?;
        if !response_body.is_empty() {
            return Err(VisionError::Protocol {
                message: "worker OCR response unexpectedly contained a binary body".to_owned(),
            });
        }
        match response.payload {
            WorkerResponse::Recognize {
                response: Some(response),
                error: None,
            } => {
                if response.request_id != request.request_id {
                    return Err(VisionError::OcrCancelled {
                        reason: "worker response request_id does not match the active request"
                            .to_owned(),
                    });
                }
                Ok(response)
            }
            WorkerResponse::Recognize {
                response: _,
                error: Some(error),
            } => {
                if error.code == "cancelled" {
                    Err(VisionError::OcrCancelled {
                        reason: error.message,
                    })
                } else if error.code == "deadline_exceeded" {
                    Err(VisionError::FrameTimeout {
                        timeout_ms: request.deadline.as_millis().min(u128::from(u64::MAX)) as u64,
                    })
                } else {
                    Err(VisionError::OcrFailed {
                        message: format!("{}: {}", error.code, error.message),
                    })
                }
            }
            WorkerResponse::Health { .. } => Err(VisionError::Protocol {
                message: "worker returned health response for OCR request".to_owned(),
            }),
            WorkerResponse::Recognize {
                response: None,
                error: None,
            } => Err(VisionError::Protocol {
                message: "worker returned empty OCR response and no error".to_owned(),
            }),
        }
    }
}
