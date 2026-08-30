//! Named Pipe 控制面的版本化 JSON DTO。

use argusflow_core::WindowIdentity;
use serde::{Deserialize, Serialize};

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect, TopologyGeneration},
    ocr::{OcrDiagnosticImageEncoding, OcrModel, OcrProfile, OcrResponse},
};

/// 当前 Rust/Python worker 协议版本。
pub const VISION_PROTOCOL_VERSION: &str = "argusflow.vision.v7";

/// 单次 Named Pipe 消息允许携带的最大原始像素体大小。
pub const MAX_PIXEL_BODY_BYTES: usize = 64 * 1024 * 1024;

/// worker 的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLifecycle {
    /// 进程已启动但尚未加载模型。
    Starting,
    /// 正在探测 CUDA 并选择实际推理设备。
    SelectingDevice,
    /// 正在加载默认 small 模型或 medium 回退模型。
    LoadingModels,
    /// 可接受请求。
    Ready,
    /// 可以继续工作但出现可观测降级。
    Degraded,
    /// 进程或模型不可用。
    Failed,
}

/// Paddle 实际使用的强类型推理设备。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkerDevice {
    /// 使用 CPU 与 oneDNN/MKLDNN 推理。
    Cpu,
    /// 使用指定索引的 CUDA 设备。
    Cuda {
        /// Paddle 可见设备中的零基索引。
        index: u32,
    },
}

/// 当前 worker 使用的推理引擎。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerInferenceEngine {
    /// Paddle 静态图预测器，保持 FP32 精度。
    PaddleStatic,
}

/// 单个 OCR 档位的加载与预热生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerModelLifecycle {
    /// 尚未开始加载该档位。
    Pending,
    /// 正在构造 PaddleOCR pipeline 并加载权重。
    Loading,
    /// 正在用真实文本图像执行首次完整推理。
    Warming,
    /// 该档位可以接受 OCR 请求。
    Ready,
    /// 该档位加载或预热失败。
    Failed,
}

/// 一个 OCR 档位的设备、引擎和就绪状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerModelInfo {
    /// ArgusFlow profile。
    pub model: OcrModel,
    /// 实际推理设备。
    pub device: WorkerDevice,
    /// inference engine。
    pub engine: WorkerInferenceEngine,
    /// 当前模型生命周期。
    pub lifecycle: WorkerModelLifecycle,
    /// 当前档位失败时可展示的稳定说明。
    pub message: Option<String>,
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
    /// Small 与 Medium 两个档位各自的初始化状态。
    pub models: Vec<WorkerModelInfo>,
    /// 当前排队任务数。
    pub queue_depth: usize,
    /// GPU 初始化失败并自动切换 CPU 等可继续工作的降级原因。
    pub degradation_reason: Option<String>,
}

impl WorkerHealth {
    /// 创建尚未完成 Named Pipe 握手的 health。
    pub fn starting() -> Self {
        Self {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            worker_version: "named-pipe-not-connected".to_owned(),
            paddleocr_version: String::new(),
            lifecycle: WorkerLifecycle::Starting,
            models: Vec::new(),
            queue_depth: 0,
            degradation_reason: None,
        }
    }

    /// 创建一个已失败且不可接受请求的 health。
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            protocol_version: VISION_PROTOCOL_VERSION.to_owned(),
            worker_version: "named-pipe-client".to_owned(),
            paddleocr_version: String::new(),
            lifecycle: WorkerLifecycle::Failed,
            models: Vec::new(),
            queue_depth: 0,
            degradation_reason: Some(message.into()),
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

/// Pagefile-backed Windows 共享内存中的 BGRA 像素平面。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedMemoryPixels {
    /// 当前登录会话内的 mapping 名称。
    pub mapping_name: String,
    /// 图片宽度。
    pub width: u32,
    /// 图片高度。
    pub height: u32,
    /// 每行字节数，允许 D3D readback padding。
    pub stride_bytes: u32,
    /// mapping 中有效像素字节数，必须等于 `stride_bytes * height`。
    pub length: u64,
    /// 当前请求持有 mapping 的唯一租约 ID。
    pub lease_id: String,
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
    /// 请求空闲或失败的 worker 执行设备选择、模型加载和预热。
    Initialize,
    /// 请求对一个共享内存 ROI 执行 OCR。
    Recognize {
        /// OCR 请求及其共享内存像素描述。
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
    /// pagefile-backed Windows 命名共享内存像素描述。
    pub pixels: SharedMemoryPixels,
    /// 请求截止时间，单位为毫秒。
    pub deadline_ms: u64,
    /// 只控制旁路诊断产物，不改变 OCR 预处理或推理参数。
    pub diagnostics: WorkerDiagnosticsRequest,
}

/// Host 对单次 OCR 请求声明的诊断产物策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDiagnosticsRequest {
    /// 是否返回真正传给模型的最终像素。
    pub capture_model_input: bool,
    /// 开启时使用的无损编码。
    pub encoding: OcrDiagnosticImageEncoding,
}

/// Recognize 响应 binary body 的类型化元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerBinaryArtifact {
    /// 当前 v6 只允许 model_input。
    pub kind: WorkerBinaryArtifactKind,
    /// binary body 的无损编码。
    pub encoding: OcrDiagnosticImageEncoding,
    /// 解码后的像素宽度。
    pub width: u32,
    /// 解码后的像素高度。
    pub height: u32,
    /// 与 framed body header 一致的字节数。
    pub body_length: u64,
    /// 小写十六进制 SHA-256。
    pub sha256: String,
}

/// Worker 响应允许携带的二进制 artifact 类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerBinaryArtifactKind {
    /// `pipeline.predict` 接收的 exact model input。
    ModelInput,
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
        /// Diagnostics 请求成功时对响应 binary body 的描述。
        artifact: Option<WorkerBinaryArtifact>,
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
    /// 从内部 OCR 请求和已经写入像素的共享内存租约创建控制面 DTO。
    pub fn from_request(
        request: &crate::ocr::OcrRequest,
        capture_model_input: bool,
        mapping_name: String,
        lease_id: String,
    ) -> Result<Self, VisionError> {
        if request.deadline.is_zero() {
            return Err(VisionError::Protocol {
                message: "OCR request deadline must be non-zero".to_owned(),
            });
        }
        let stride_bytes =
            u32::try_from(request.image.stride_bytes).map_err(|_| VisionError::Protocol {
                message: "OCR image stride does not fit worker protocol".to_owned(),
            })?;
        let minimum_stride =
            request
                .image
                .width
                .checked_mul(4)
                .ok_or_else(|| VisionError::Protocol {
                    message: "OCR image width overflows BGRA stride".to_owned(),
                })?;
        let expected_bytes = u64::from(stride_bytes)
            .checked_mul(u64::from(request.image.height))
            .ok_or_else(|| VisionError::Protocol {
                message: "OCR image byte length overflows worker protocol".to_owned(),
            })?;
        let expected_bytes_usize =
            usize::try_from(expected_bytes).map_err(|_| VisionError::Protocol {
                message: "OCR image byte length does not fit host memory".to_owned(),
            })?;
        if request.image.width == 0
            || request.image.height == 0
            || stride_bytes < minimum_stride
            || expected_bytes_usize > MAX_PIXEL_BODY_BYTES
        {
            return Err(VisionError::Protocol {
                message: "OCR image dimensions, stride, or body size are invalid".to_owned(),
            });
        }
        let pixels = request.image.pixels();
        if pixels.len() != expected_bytes_usize {
            return Err(VisionError::Protocol {
                message: "OCR image buffer length must equal stride*height".to_owned(),
            });
        }
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
            pixels: SharedMemoryPixels {
                mapping_name,
                width: request.image.width,
                height: request.image.height,
                stride_bytes,
                length: expected_bytes,
                lease_id,
            },
            deadline_ms,
            diagnostics: WorkerDiagnosticsRequest {
                capture_model_input,
                encoding: OcrDiagnosticImageEncoding::Png,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        frame::{FrameId, PixelFormat, TopologyGeneration},
        image::PixelImage,
        ocr::{OcrRequest, OcrRequestId},
    };

    fn request(width: u32, height: u32, stride_bytes: usize, pixels: Vec<u8>) -> OcrRequest {
        OcrRequest {
            request_id: OcrRequestId::new(),
            window: WindowIdentity {
                handle: 11,
                process_id: 22,
            },
            frame_id: FrameId::new(3),
            topology_generation: TopologyGeneration::new(4),
            profile: OcrProfile::small(),
            roi: PhysicalRect::new(0, 0, width, height).expect("fixture ROI is non-empty"),
            image: PixelImage::new(width, height, stride_bytes, PixelFormat::Bgra8Unorm, pixels)
                .expect("fixture image is valid"),
            deadline: std::time::Duration::from_secs(1),
        }
    }

    #[test]
    fn binary_body_preserves_large_roi_without_json_pixel_array() {
        let wire_request = request(1_200, 800, 1_200 * 4, vec![17; 1_200 * 800 * 4]);
        let wire = WorkerOcrRequest::from_request(
            &wire_request,
            false,
            "Local\\argusflow-test".to_owned(),
            "lease".to_owned(),
        )
        .expect("large ROI should fit shared memory");

        assert_eq!(wire.pixels.width, 1_200);
        assert_eq!(wire.pixels.height, 800);
        assert_eq!(wire.pixels.length, 3_840_000);
        let encoded = serde_json::to_string(&wire).expect("control DTO should serialize");
        assert!(!encoded.contains("17,17,17"));
    }

    #[test]
    fn stride_padding_is_kept_in_the_binary_body() {
        let request = request(4, 2, 20, vec![0; 40]);
        let wire = WorkerOcrRequest::from_request(
            &request,
            false,
            "Local\\argusflow-test".to_owned(),
            "lease".to_owned(),
        )
        .expect("row padding should be a valid transport detail");

        assert_eq!(wire.pixels.stride_bytes, 20);
        assert_eq!(wire.pixels.length, 40);
    }

    #[test]
    fn extra_pixel_storage_is_rejected_instead_of_silently_truncated() {
        let request = request(4, 2, 16, vec![0; 40]);

        let error = WorkerOcrRequest::from_request(
            &request,
            false,
            "Local\\argusflow-test".to_owned(),
            "lease".to_owned(),
        )
        .expect_err("body length must match stride*height exactly");
        assert!(matches!(error, VisionError::Protocol { .. }));
    }
}
