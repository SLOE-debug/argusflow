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
    client::{read_framed_json, write_framed_json},
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
        let response = self.round_trip(envelope, Duration::from_secs(3)).await?;
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
        let mut health = self
            .health
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *health = WorkerHealth::failed(message);
    }

    /// 通过一条连接完成完整 framed request/response 交换。
    async fn round_trip(
        &self,
        envelope: WorkerProtocolEnvelope<WorkerCommand>,
        timeout: Duration,
    ) -> Result<WorkerProtocolEnvelope<WorkerResponse>, VisionError> {
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
        let client = connection
            .as_mut()
            .ok_or_else(|| VisionError::WorkerUnavailable {
                message: "vision worker pipe connection was not installed".to_owned(),
            })?;
        let exchange = async {
            write_framed_json(client, &envelope).await?;
            read_framed_json(client).await
        };
        let response: WorkerProtocolEnvelope<WorkerResponse> =
            match tokio::time::timeout(timeout, exchange).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    *connection = None;
                    self.mark_failed(error.to_string());
                    return Err(error);
                }
                Err(_) => {
                    *connection = None;
                    let error = VisionError::FrameTimeout {
                        timeout_ms: timeout.as_millis() as u64,
                    };
                    self.mark_failed(error.to_string());
                    return Err(error);
                }
            };
        if response.protocol_version != VISION_PROTOCOL_VERSION
            || response.session_token != self.session_token
        {
            *connection = None;
            let error = VisionError::Protocol {
                message: "worker protocol version or session token mismatch".to_owned(),
            };
            self.mark_failed(error.to_string());
            return Err(error);
        }
        if response.request_id != envelope.request_id {
            *connection = None;
            let error = VisionError::Protocol {
                message: "worker response request_id does not match the active envelope".to_owned(),
            };
            self.mark_failed(error.to_string());
            return Err(error);
        }
        if let WorkerResponse::Health { ref health } = response.payload {
            let mut current = self
                .health
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *current = health.clone();
        }
        Ok(response)
    }
}

#[async_trait]
impl OcrEngine for NamedPipeOcrEngine {
    fn health(&self) -> WorkerHealth {
        NamedPipeOcrEngine::health(self)
    }

    async fn recognize(&self, request: OcrRequest) -> Result<OcrResponse, VisionError> {
        let wire_request = WorkerOcrRequest::from_request(&request)?;
        let envelope = WorkerProtocolEnvelope {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            request_id: wire_request.request_id.clone(),
            session_token: self.session_token.clone(),
            payload: WorkerCommand::Recognize {
                request: wire_request,
            },
        };
        let response = self.round_trip(envelope, request.deadline).await?;
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
