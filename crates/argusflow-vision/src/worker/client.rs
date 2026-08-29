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

use super::{
    WorkerHealth, WorkerLifecycle,
    protocol::{MAX_PIXEL_BODY_BYTES, VISION_PROTOCOL_VERSION},
};

/// framed message 单帧的最大控制面 payload，防止错误 peer 申请无界内存。
pub const MAX_CONTROL_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// binary body 帧头魔数，避免 v1 四字节长度帧被误读。
const FRAME_MAGIC: [u8; 4] = *b"AFV2";

/// binary body 帧头长度：magic、控制面长度和像素体长度。
const FRAME_HEADER_BYTES: usize = 16;

/// 读取一条带有控制面 JSON 和可选 binary body 的版本化消息。
pub async fn read_framed_message<R, T>(reader: &mut R) -> Result<(T, Vec<u8>), VisionError>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to read frame header: {error}"),
        })?;
    if header[..4] != FRAME_MAGIC {
        return Err(VisionError::Protocol {
            message: "invalid vision worker frame magic".to_owned(),
        });
    }
    let control_len =
        u32::from_le_bytes(header[4..8].try_into().map_err(|_| VisionError::Protocol {
            message: "invalid control length field".to_owned(),
        })?) as usize;
    let body_len =
        u64::from_le_bytes(
            header[8..16]
                .try_into()
                .map_err(|_| VisionError::Protocol {
                    message: "invalid binary body length field".to_owned(),
                })?,
        );
    if control_len > MAX_CONTROL_FRAME_BYTES {
        return Err(VisionError::Protocol {
            message: format!("control frame is too large: {control_len} bytes"),
        });
    }
    let body_len = usize::try_from(body_len).map_err(|_| VisionError::Protocol {
        message: "binary body length does not fit host memory".to_owned(),
    })?;
    if body_len > MAX_PIXEL_BODY_BYTES {
        return Err(VisionError::Protocol {
            message: format!("binary body is too large: {body_len} bytes"),
        });
    }
    let mut payload = vec![0_u8; control_len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to read control payload: {error}"),
        })?;
    let message = serde_json::from_slice(&payload).map_err(|error| VisionError::Protocol {
        message: format!("invalid control JSON: {error}"),
    })?;
    let mut body = vec![0_u8; body_len];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to read binary body: {error}"),
        })?;
    Ok((message, body))
}

/// 写入一条带有控制面 JSON 和可选 binary body 的版本化消息。
pub async fn write_framed_message<W, T>(
    writer: &mut W,
    value: &T,
    body: &[u8],
) -> Result<(), VisionError>
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
    if body.len() > MAX_PIXEL_BODY_BYTES {
        return Err(VisionError::Protocol {
            message: format!("binary body is too large: {} bytes", body.len()),
        });
    }
    let control_len = u32::try_from(payload.len()).map_err(|_| VisionError::Protocol {
        message: "control payload length does not fit frame header".to_owned(),
    })?;
    let body_len = u64::try_from(body.len()).map_err(|_| VisionError::Protocol {
        message: "binary body length does not fit frame header".to_owned(),
    })?;
    writer
        .write_all(&FRAME_MAGIC)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write frame magic: {error}"),
        })?;
    writer
        .write_u32_le(control_len)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write control frame length: {error}"),
        })?;
    writer
        .write_u64_le(body_len)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write binary body length: {error}"),
        })?;
    writer
        .write_all(&payload)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write control payload: {error}"),
        })?;
    writer
        .write_all(body)
        .await
        .map_err(|error| VisionError::Protocol {
            message: format!("failed to write binary body: {error}"),
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
    /// 创建一个已就绪的静态测试 worker。
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{read_framed_message, write_framed_message};

    #[tokio::test]
    async fn framed_control_and_binary_body_round_trip_separately() {
        let (mut writer, mut reader) = tokio::io::duplex(1024);
        let body = vec![0_u8, 1, 2, 255];
        write_framed_message(&mut writer, &json!({"kind": "recognize"}), &body)
            .await
            .expect("frame should be writable");

        let (control, received_body): (Value, Vec<u8>) = read_framed_message(&mut reader)
            .await
            .expect("frame should be readable");
        assert_eq!(control, json!({"kind": "recognize"}));
        assert_eq!(received_body, body);
    }
}
