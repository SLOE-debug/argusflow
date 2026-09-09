//! 服务自身的历史、元数据和请求预算。
use argusflow_capture_contracts::{CaptureError, CaptureResult, PixelRect};
use argusflow_core::FailureKind;
use std::time::Duration;

/// 服务设置；原生 GPU/CPU 预算在 BackendConfig 中配置。
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// 未固定历史的保留时长。
    pub history: Duration,
    /// 同时保留的每来源版本上限。
    pub history_versions: usize,
    /// 变化日志元数据字节预算；默认与原生事件桥的 4 MiB 合计 16 MiB。
    pub metadata_bytes: usize,
    /// 同时执行的观察上限。
    pub observations: usize,
    /// 同时执行的区域采样上限。
    pub reads: usize,
    /// 显式锚点有效期。
    pub anchor_lifetime: Duration,
}
impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            history: Duration::from_secs(1),
            history_versions: 1024,
            metadata_bytes: 12 * 1024 * 1024,
            observations: 8,
            reads: 32,
            anchor_lifetime: Duration::from_secs(5),
        }
    }
}
impl CaptureConfig {
    pub(crate) fn validate(&self) -> CaptureResult<()> {
        if self.history.is_zero()
            || self.history > Duration::from_secs(60)
            || self.history_versions < 2
            || self.history_versions > 4096
            || self.metadata_bytes < 4096
            || self.metadata_bytes > 256 * 1024 * 1024
            || self.observations == 0
            || self.observations > 64
            || self.reads == 0
            || self.reads > 256
            || self.anchor_lifetime.is_zero()
            || self.anchor_lifetime > Duration::from_secs(60)
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "capture_config",
                "采样服务配置超限",
            ));
        }
        Ok(())
    }
}
/// 一个交互的视觉观察策略，忽略区域只作用于该请求。
#[derive(Debug, Clone)]
pub struct ObservationOptions {
    /// 至少观察多久，避免过早把响应前静止当作最终结果。
    pub minimum: Duration,
    /// 连续无真实变化的时间。
    pub quiet: Duration,
    /// 稳定等待总上限。
    pub maximum: Duration,
    /// 结果读取区域的物理像素外扩。
    pub padding: u32,
    /// 图像本地显式忽略区。
    pub ignored: Vec<PixelRect>,
}
impl Default for ObservationOptions {
    fn default() -> Self {
        Self {
            minimum: Duration::from_millis(300),
            quiet: Duration::from_millis(150),
            maximum: Duration::from_secs(2),
            padding: 16,
            ignored: Vec::new(),
        }
    }
}
impl ObservationOptions {
    pub(crate) fn validate(&self) -> CaptureResult<()> {
        if self.quiet.is_zero()
            || self.maximum < self.minimum
            || self.maximum < self.quiet
            || self.maximum > Duration::from_secs(30)
            || self.padding > 4096
            || self.ignored.len() > 128
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "observation_options",
                "观察时间或区域数量超限",
            ));
        }
        Ok(())
    }
}
