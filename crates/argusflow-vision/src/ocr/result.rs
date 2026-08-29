//! OCR 输入输出 DTO；不在 worker 中计算业务布局或动作选择。

use std::{fmt, time::Duration};

use argusflow_core::WindowIdentity;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect, TopologyGeneration},
    image::{CapturedFrame, PixelImage},
    worker::WorkerHealth,
};

/// 当前支持的 PaddleOCR 模型档位；Core backend 不绑定厂商 SKU。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrModel {
    /// PP-OCRv6 tiny，高频增量 ROI 识别。
    PpOcrV6Tiny,
    /// PP-OCRv6 small，桌面 GUI 的默认精度/延迟平衡档。
    PpOcrV6Small,
    /// PP-OCRv6 medium，低置信度升级和关键验证。
    PpOcrV6Medium,
}

impl OcrModel {
    /// 返回模型对应的 ArgusFlow worker profile 名称。
    pub const fn profile_name(self) -> &'static str {
        match self {
            Self::PpOcrV6Tiny => "pp_ocr_v6_tiny",
            Self::PpOcrV6Small => "pp_ocr_v6_small",
            Self::PpOcrV6Medium => "pp_ocr_v6_medium",
        }
    }

    /// 返回运行日志和诊断界面使用的模型名称。
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::PpOcrV6Tiny => "PP-OCRv6 Tiny",
            Self::PpOcrV6Small => "PP-OCRv6 Small",
            Self::PpOcrV6Medium => "PP-OCRv6 Medium",
        }
    }
}

/// OCR 结果来源，用于 scene provenance 和 Evidence 解释。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrSource {
    /// PP-OCRv6 tiny 识别结果。
    OcrTiny,
    /// PP-OCRv6 small 识别结果。
    OcrSmall,
    /// PP-OCRv6 medium 识别结果。
    OcrMedium,
    /// 根据几何或重复节奏推断的布局信息。
    LayoutHeuristic,
    /// UIA 外层语义投影。
    UiaProjection,
    /// 最后的无文字 GUI grounding 结果。
    GuiGrounding,
}

impl From<OcrModel> for OcrSource {
    fn from(model: OcrModel) -> Self {
        match model {
            OcrModel::PpOcrV6Tiny => Self::OcrTiny,
            OcrModel::PpOcrV6Small => Self::OcrSmall,
            OcrModel::PpOcrV6Medium => Self::OcrMedium,
        }
    }
}

/// OCR 请求的强类型 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OcrRequestId(Uuid);

impl OcrRequestId {
    /// 创建一个新的请求 ID。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 从已有 UUID 恢复请求 ID，供协议反序列化使用。
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// 返回底层 UUID。
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for OcrRequestId {
    fn default() -> Self {
        Self::new()
    }
}

/// OCR polygon 中的帧本地物理像素点。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PolygonPoint {
    /// 水平坐标。
    pub x: f32,
    /// 垂直坐标。
    pub y: f32,
}

/// 截图 ROI 进入 OCR 前可选的图像预处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrImagePreprocessing {
    /// 保持截图像素不变，适合调用方已经完成图像优化的输入。
    None,
    /// 对小型桌面文字 ROI 做有像素上限的放大、局部对比度增强和轻量锐化。
    AdaptiveDesktopText,
}

/// 一次识别请求的模型和预处理选项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrOptions {
    /// OCR 语言 profile，例如 `ch` 或 `en`。
    pub language: String,
    /// GUI 文本默认关闭文档方向分类。
    pub use_doc_orientation_classify: bool,
    /// GUI 文本默认关闭文档去畸变。
    pub use_doc_unwarping: bool,
    /// GUI 文本默认关闭 textline orientation。
    pub use_textline_orientation: bool,
    /// 截图完成后、PaddleOCR 检测前使用的图像预处理策略。
    pub image_preprocessing: OcrImagePreprocessing,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            language: "ch".to_owned(),
            use_doc_orientation_classify: false,
            use_doc_unwarping: false,
            use_textline_orientation: false,
            image_preprocessing: OcrImagePreprocessing::AdaptiveDesktopText,
        }
    }
}

/// 一次识别请求绑定的模型 profile。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrProfile {
    /// 识别模型档位。
    pub model: OcrModel,
    /// 模型的预处理设置。
    pub options: OcrOptions,
}

impl OcrProfile {
    /// 创建默认 tiny profile。
    pub fn tiny() -> Self {
        Self {
            model: OcrModel::PpOcrV6Tiny,
            options: OcrOptions::default(),
        }
    }

    /// 创建桌面 GUI 默认使用的 small profile。
    pub fn small() -> Self {
        Self {
            model: OcrModel::PpOcrV6Small,
            options: OcrOptions::default(),
        }
    }

    /// 创建默认 medium profile。
    pub fn medium() -> Self {
        Self {
            model: OcrModel::PpOcrV6Medium,
            options: OcrOptions::default(),
        }
    }
}

/// Worker 对单个 OCR ROI 实际执行的图像预处理摘要。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrPreprocessingSummary {
    /// 原始 ROI 宽度。
    pub input_width: u32,
    /// 原始 ROI 高度。
    pub input_height: u32,
    /// 送入 PaddleOCR 的图像宽度。
    pub output_width: u32,
    /// 送入 PaddleOCR 的图像高度。
    pub output_height: u32,
    /// 是否执行了局部对比度增强。
    pub contrast_enhanced: bool,
    /// 是否执行了轻量锐化。
    pub sharpened: bool,
}

impl OcrPreprocessingSummary {
    /// 返回几何放大比例的千分值，避免指标和摘要依赖浮点相等判断。
    pub fn scale_milli(self) -> u32 {
        if self.input_width == 0 || self.input_height == 0 {
            return 1_000;
        }
        let width_scale = u64::from(self.output_width) * 1_000 / u64::from(self.input_width);
        let height_scale = u64::from(self.output_height) * 1_000 / u64::from(self.input_height);
        u32::try_from(width_scale.min(height_scale)).unwrap_or(u32::MAX)
    }

    /// 判断本次请求是否改变了 OCR 输入像素。
    pub const fn was_applied(self) -> bool {
        self.input_width != self.output_width
            || self.input_height != self.output_height
            || self.contrast_enhanced
            || self.sharpened
    }
}

/// 发送给 OCR worker 的一次 ROI 任务。
#[derive(Clone)]
pub struct OcrRequest {
    /// 请求 ID，用于取消、ack 和 evidence 关联。
    pub request_id: OcrRequestId,
    /// 请求所在的 AppSession 窗口身份。
    pub window: WindowIdentity,
    /// 请求使用的帧。
    pub frame_id: FrameId,
    /// 请求所在的拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// 模型 profile。
    pub profile: OcrProfile,
    /// 帧本地 ROI。
    pub roi: PhysicalRect,
    /// 短期内存中的 ROI 像素。
    pub image: PixelImage,
    /// 单次请求的截止时间。
    pub deadline: Duration,
}

impl fmt::Debug for OcrRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OcrRequest")
            .field("request_id", &self.request_id)
            .field("window", &self.window)
            .field("frame_id", &self.frame_id)
            .field("topology_generation", &self.topology_generation)
            .field("profile", &self.profile)
            .field("roi", &self.roi)
            .field("image", &self.image)
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl OcrRequest {
    /// 从当前帧复制指定 ROI，建立不会被 capture buffer 复用影响的请求。
    pub fn from_frame(
        window: WindowIdentity,
        frame_id: FrameId,
        topology_generation: TopologyGeneration,
        frame: &CapturedFrame,
        roi: PhysicalRect,
        profile: OcrProfile,
        deadline: Duration,
    ) -> Result<Self, VisionError> {
        if frame.window != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(frame.window),
            });
        }
        if frame.frame_id != frame_id || frame.topology_generation != topology_generation {
            return Err(VisionError::OcrCancelled {
                reason: "OCR request metadata does not match the source frame".to_owned(),
            });
        }
        if deadline.is_zero() {
            return Err(VisionError::Protocol {
                message: "OCR request deadline must be non-zero".to_owned(),
            });
        }
        Ok(Self {
            request_id: OcrRequestId::new(),
            window,
            frame_id,
            topology_generation,
            profile,
            roi,
            image: frame.crop(roi)?,
            deadline,
        })
    }
}

/// worker 返回的一个 OCR 文本框。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrItem {
    /// OCR 原始文本，禁止在 worker 之外静默纠正。
    pub raw_text: String,
    /// 0 到 1 的识别置信度。
    pub confidence: f32,
    /// 帧本地 polygon；bbox 由 Rust 统一计算。
    pub polygon: Vec<PolygonPoint>,
}

/// 一次 OCR 请求的完整响应。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrResponse {
    /// 与请求一一对应的请求 ID。
    pub request_id: OcrRequestId,
    /// worker 处理的帧 ID。
    pub frame_id: FrameId,
    /// worker 处理的拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// 实际使用的模型。
    pub model: OcrModel,
    /// worker 处理耗时。
    pub elapsed_ms: u64,
    /// Worker 实际执行的几何与图像增强摘要。
    pub preprocessing: OcrPreprocessingSummary,
    /// 识别结果。
    pub items: Vec<OcrItem>,
}

/// Rust runtime 到 OCR worker 的最小异步行为契约。
#[async_trait]
pub trait OcrEngine: fmt::Debug + Send + Sync {
    /// 返回当前 worker health，供 Planner availability 使用。
    fn health(&self) -> WorkerHealth;

    /// 在 request deadline 内完成一次 OCR。
    async fn recognize(&self, request: OcrRequest) -> Result<OcrResponse, VisionError>;
}

/// 规范化用于查询和 stable hash 的文本，同时保留 `raw_text`。
pub fn normalize_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '\u{200b}' | '\u{feff}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_is_deterministic_and_preserves_raw_contract() {
        assert_eq!(normalize_text("  项目\u{200b}  讨论\n群  "), "项目 讨论 群");
    }
}
