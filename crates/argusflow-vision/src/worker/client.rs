//! OCR worker 客户端适配和内存测试实现。

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    error::VisionError,
    ocr::{OcrEngine, OcrRequest, OcrResponse},
};

use super::{WorkerHealth, WorkerLifecycle, protocol::VISION_PROTOCOL_VERSION};

/// framed JSON 单帧的最大控制面 payload，防止错误 peer 申请无界内存。
const MAX_CONTROL_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// 通过 4 字节 little-endian 长度前缀读一条 JSON 消息。
pub async fn read_framed_json<R, T>(reader: &mut R) -> Result<T, VisionError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let frame_len = reader
        .read_u32_le()
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to read frame length: {error}"),
        })? as usize;
    if frame_len > MAX_CONTROL_FRAME_BYTES {
        return Err(VisionError::Protocol {
            message: format!("control frame is too large: {frame_len} bytes"),
        });
    }
    let mut payload = vec![0_u8; frame_len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to read control payload: {error}"),
        })?;
    serde_json::from_slice(&payload).map_err(|error| VisionError::Protocol {
        message: format!("invalid control JSON: {error}"),
    })
}

/// 通过 4 字节 little-endian 长度前缀写一条 JSON 消息。
pub async fn write_framed_json<W, T>(writer: &mut W, value: &T) -> Result<(), VisionError>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(value).map_err(|error| VisionError::Protocol {
        message: format!("failed to encode control JSON: {error}"),
    })?;
    if payload.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(VisionError::Protocol {
            message: format!("control frame is too large: {} bytes", payload.len()),
        });
    }
    writer
        .write_u32_le(payload.len() as u32)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write control frame length: {error}"),
        })?;
    writer
        .write_all(&payload)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write control payload: {error}"),
        })?;
    writer.flush().await.map_err(|error| VisionError::Protocol {
        message: format!("failed to flush control JSON: {error}"),
    })
}

/// 对真正的 Named Pipe client 或 test double 的统一 worker 外观。
#[derive(Clone)]
pub struct VisionWorkerClient {
    /// 具体 OCR transport/engine。
    inner: Arc<dyn OcrEngine>,
}

impl std::fmt::Debug for VisionWorkerClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VisionWorkerClient")
            .field("health", &self.health())
            .finish()
    }
}

impl VisionWorkerClient {
    /// 把一个底层 engine 包装成可共享的 worker client。
    pub fn new(inner: Arc<dyn OcrEngine>) -> Self {
        Self { inner }
    }

    /// 返回 worker 当前 health。
    pub fn health(&self) -> WorkerHealth {
        self.inner.health()
    }
}

#[async_trait]
impl OcrEngine for VisionWorkerClient {
    fn health(&self) -> WorkerHealth {
        self.inner.health()
    }

    async fn recognize(&self, request: OcrRequest) -> Result<OcrResponse, VisionError> {
        self.inner.recognize(request).await
    }
}

/// 显式失败的 worker 实现，用于没有安装本地模型时准确报告 availability。
#[derive(Debug, Clone)]
pub struct UnavailableOcrEngine {
    /// 失败原因与版本信息。
    health: WorkerHealth,
}

impl UnavailableOcrEngine {
    /// 用稳定错误摘要创建未就绪 worker。
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            health: WorkerHealth::failed(message),
        }
    }
}

#[async_trait]
impl OcrEngine for UnavailableOcrEngine {
    fn health(&self) -> WorkerHealth {
        self.health.clone()
    }

    async fn recognize(&self, _request: OcrRequest) -> Result<OcrResponse, VisionError> {
        Err(VisionError::WorkerUnavailable {
            message: self.health.worker_version.clone(),
        })
    }
}

/// 按帧预置 OCR 响应的 deterministic engine，仅用于 golden/unit test。
#[derive(Debug, Clone)]
pub struct StaticOcrEngine {
    /// 固定 health。
    health: WorkerHealth,
    /// 依次返回的响应队列。
    responses: Arc<Mutex<VecDeque<Result<OcrResponse, VisionError>>>>,
}

impl StaticOcrEngine {
    /// 创建一个已就绪的 static tiny worker。
    pub fn new(responses: impl IntoIterator<Item = OcrResponse>) -> Self {
        Self {
            health: WorkerHealth {
                protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
                worker_version: "static-test-worker".to_owned(),
                paddleocr_version: "3.7.0".to_owned(),
                lifecycle: WorkerLifecycle::Ready,
                model: None,
                queue_depth: 0,
            },
            responses: Arc::new(Mutex::new(
                responses.into_iter().map(Ok).collect::<VecDeque<_>>(),
            )),
        }
    }

    /// 将一次显式失败插入响应队列。
    pub fn push_error(&self, error: VisionError) {
        self.responses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push_back(Err(error));
    }
}

#[async_trait]
impl OcrEngine for StaticOcrEngine {
    fn health(&self) -> WorkerHealth {
        self.health.clone()
    }

    async fn recognize(&self, request: OcrRequest) -> Result<OcrResponse, VisionError> {
        let mut response = self
            .responses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
            .ok_or_else(|| VisionError::OcrFailed {
                message: "static OCR response queue is empty".to_owned(),
            })??;
        if response.frame_id != request.frame_id
            || response.topology_generation != request.topology_generation
        {
            return Err(VisionError::OcrCancelled {
                reason: "static response does not match the current request generation".to_owned(),
            });
        }
        let _deadline = request.deadline.min(Duration::from_secs(60));
        response.request_id = request.request_id;
        Ok(response)
    }
}
