//! Named Pipe 控制面的版本化 JSON DTO。

use argusflow_core::WindowIdentity;
use serde::{Deserialize, Serialize};

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect, TopologyGeneration},
    ocr::{OcrModel, OcrProfile, OcrResponse},
};

/// 当前 Rust/Python worker 协议版本。
pub const VISION_PROTOCOL_VERSION: &str = "argusflow.vision.v1";

/// worker 的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycle {
    /// 进程已启动但尚未加载模型。
    Starting,
    /// 正在加载 tiny 或 medium 模型。
    LoadingModels,
    /// 可接受请求。
    Ready,
    /// 可以继续工作但出现可观测降级。
    Degraded,
    /// 进程或模型不可用。
    Failed,
}

/// 当前 worker 使用的模型和 inference engine 摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerModelInfo {
    /// ArgusFlow profile。
    pub model: OcrModel,
    /// 设备，例如 `cpu` 或 `gpu:0`。
    pub device: String,
    /// inference engine 及其版本摘要。
    pub engine: String,
}

/// worker health，进入 Planner Explain 和 Evidence manifest。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHealth {
    /// 协议版本。
    pub protocol_version: String,
    /// worker 自身构建版本。
    pub worker_version: String,
    /// PaddleOCR 版本。
    pub paddleocr_version: String,
    /// 当前状态。
    pub lifecycle: WorkerLifecycle,
    /// 当前默认模型信息；lazy load 时可以为空。
    pub model: Option<WorkerModelInfo>,
    /// 当前排队任务数。
    pub queue_depth: usize,
}

impl WorkerHealth {
    /// 创建尚未完成 Named Pipe 握手的 health。
    pub fn starting() -> Self {
        Self {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            worker_version: "named-pipe-not-connected".to_owned(),
            paddleocr_version: String::new(),
            lifecycle: WorkerLifecycle::Starting,
            model: None,
            queue_depth: 0,
        }
    }

    /// 创建一个已失败且不可接受请求的 health。
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            worker_version: message.into(),
            paddleocr_version: String::new(),
            lifecycle: WorkerLifecycle::Failed,
            model: None,
            queue_depth: 0,
        }
    }

    /// 判断 health 是否允许处理 OCR。
    pub const fn is_ready(&self) -> bool {
        matches!(
            self.lifecycle,
            WorkerLifecycle::Ready | WorkerLifecycle::Degraded
        )
    }
}

/// 像素平面传输方式；P0 支持小 ROI inline，P1 可切换共享内存 lease。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PixelTransport {
    /// 小 ROI 直接放入 framed message 的 bytes。
    InlineBytes {
        /// BGRA 字节数组；P0 使用 framed JSON 直接传输。
        bytes: Vec<u8>,
        /// 图片宽度。
        width: u32,
        /// 图片高度。
        height: u32,
        /// 行步长。
        stride_bytes: u32,
    },
    /// 共享内存 ring slot 的租约描述。
    SharedMemory {
        /// 当前用户 ACL 下的 mapping 名称。
        mapping_name: String,
        /// slot 起始偏移。
        offset: u64,
        /// slot 有效字节数。
        length: u64,
        /// Rust 等待 worker ack 前使用的租约 token。
        lease_id: String,
    },
}

/// Named Pipe 中所有请求和响应共用的版本化 envelope。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerProtocolEnvelope<T> {
    /// 版本字段必须在 decode 后首先校验。
    pub protocol_version: String,
    /// Rust 与 worker 端关联请求的 UUID 字符串。
    pub request_id: String,
    /// 由当前应用会话生成并传给 worker 的随机 token。
    pub session_token: String,
    /// 具体消息 payload。
    pub payload: T,
}

/// Named Pipe 控制面请求类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerCommand {
    /// 请求当前 worker health 和版本信息。
    Health,
    /// 请求对一个 ROI 执行 OCR。
    Recognize {
        /// OCR 请求及其 inline/shared-memory 像素描述。
        request: WorkerOcrRequest,
    },
}

/// 可跨 Rust/Python 边界序列化的 OCR 请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOcrRequest {
    /// 请求 UUID 字符串。
    pub request_id: String,
    /// 目标 HWND/PID 身份。
    pub window: WindowIdentity,
    /// 绑定的帧 ID。
    pub frame_id: FrameId,
    /// 绑定的窗口拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// OCR 模型 profile。
    pub profile: OcrProfile,
    /// 帧本地 ROI。
    pub roi: PhysicalRect,
    /// P0 inline 或 P1 shared-memory 像素传输描述。
    pub pixel_transport: PixelTransport,
    /// 请求截止时间，单位为毫秒。
    pub deadline_ms: u64,
}

/// Named Pipe 控制面响应类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerResponse {
    /// 返回 worker health。
    Health {
        /// 当前 worker health。
        health: WorkerHealth,
    },
    /// 返回 OCR 结果或结构化错误。
    Recognize {
        /// 成功时的完整 OCR 响应。
        response: Option<OcrResponse>,
        /// 失败时的 worker 错误。
        error: Option<WorkerError>,
    },
}

/// worker 可解释的单次请求错误。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerError {
    /// 稳定错误码。
    pub code: String,
    /// 不含屏幕文本的错误说明。
    pub message: String,
}

impl WorkerOcrRequest {
    /// 从内部 OCR 请求创建可传输 DTO；P0 直接复制短期 ROI 像素。
    pub fn from_request(request: &crate::ocr::OcrRequest) -> Result<Self, VisionError> {
        if request.deadline.is_zero() {
            return Err(VisionError::Protocol {
                message: "OCR request deadline must be non-zero".to_owned(),
            });
        }
        let stride_bytes =
            u32::try_from(request.image.stride_bytes).map_err(|_| VisionError::Protocol {
                message: "OCR image stride does not fit worker protocol".to_owned(),
            })?;
        let deadline_ms = u64::try_from(request.deadline.as_millis().max(1)).map_err(|_| {
            VisionError::Protocol {
                message: "OCR request deadline does not fit worker protocol".to_owned(),
            }
        })?;
        Ok(Self {
            request_id: request.request_id.as_uuid().to_string(),
            window: request.window,
            frame_id: request.frame_id,
            topology_generation: request.topology_generation,
            profile: request.profile.clone(),
            roi: request.roi,
            pixel_transport: PixelTransport::InlineBytes {
                bytes: request.image.pixels().to_vec(),
                width: request.image.width,
                height: request.image.height,
                stride_bytes,
            },
            deadline_ms,
        })
    }
}
