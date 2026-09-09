//! 明确的模型、设备与资源预算。
use crate::OcrError as Failure;
use argusflow_core::FailureKind;
use std::path::PathBuf;

/// 官方 PP-OCRv6 模型档位。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ModelTier {
    /// 轻量模型，默认选择。
    #[default]
    Small,
    /// 较高识别质量模型。
    Medium,
}
impl ModelTier {
    pub(crate) fn directory(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
        }
    }
}

/// 明确选择推理设备；初始化失败不自动改成其他设备。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Device {
    /// 使用 ONNX Runtime CPU EP。
    #[default]
    Cpu,
    /// 使用 NVIDIA CUDA EP；device_id 必须非负。
    Cuda {
        /// CUDA 设备序号。
        device_id: i32,
    },
}

/// 一个实例固定使用一个模型档位和一个设备。
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// prepare-native-deps.ps1 生成的 .deps 目录。
    pub dependencies: PathBuf,
    /// 可选的 ORT DLL 目录；同进程同时使用 CPU/CUDA 时统一指向 GPU runtime。
    pub runtime_directory: Option<PathBuf>,
    /// 模型档位。
    pub tier: ModelTier,
    /// 推理设备。
    pub device: Device,
    /// 推理内部 CPU 线程数，不控制实例数量。
    pub cpu_threads: usize,
    /// 等待识别的队列容量，默认 2。
    pub queue_capacity: usize,
    /// 解码图片最大像素数，默认 1600 万。
    pub max_pixels: u64,
    /// 编码图片最大字节数，默认 64 MiB。
    pub max_encoded_bytes: usize,
    /// 检测输入短边最小值，默认官方配置 736。
    pub detection_min_side: u32,
    /// 检测输入长边最大值，默认官方限制 4000。
    pub detection_max_side: u32,
    /// 最多处理的检测候选数，默认 3000。
    pub max_candidates: usize,
    /// 识别文本块最低置信度，默认 0.35。
    pub minimum_confidence: f32,
}
impl OcrConfig {
    /// 使用 Small + CPU 创建默认配置；不触发下载或加载。
    pub fn new(dependencies: impl Into<PathBuf>) -> Self {
        Self {
            dependencies: dependencies.into(),
            runtime_directory: None,
            tier: ModelTier::Small,
            device: Device::Cpu,
            cpu_threads: 4,
            queue_capacity: 2,
            max_pixels: 16_000_000,
            max_encoded_bytes: 64 * 1024 * 1024,
            detection_min_side: 736,
            detection_max_side: 4000,
            max_candidates: 3000,
            minimum_confidence: 0.35,
        }
    }
    pub(crate) fn validate(&self) -> Result<(), Failure> {
        if matches!(self.device, Device::Cuda { device_id } if device_id < 0)
            || self.cpu_threads == 0
            || self.cpu_threads > 64
            || self.queue_capacity == 0
            || self.queue_capacity > 64
            || self.max_pixels == 0
            || self.max_pixels > 64_000_000
            || self.max_encoded_bytes == 0
            || self.max_encoded_bytes > 256 * 1024 * 1024
            || self.detection_min_side < 32
            || self.detection_max_side < self.detection_min_side
            || self.detection_max_side > 4000
            || self.max_candidates == 0
            || self.max_candidates > 3000
            || !self.minimum_confidence.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_confidence)
        {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "ocr_config",
                "OCR 配置超出支持范围",
            ));
        }
        Ok(())
    }
}
