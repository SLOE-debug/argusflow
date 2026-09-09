//! OCR 等消费者所需的最小区域采样接口。
use crate::{CaptureFuture, ClockTime, PixelImage, PixelRect, Snapshot, SourceId, Version};
use argusflow_core::Operation;
use std::{sync::Arc, time::Duration};

/// 一份已确认区域内容的比较令牌，固定图像资源直到释放或来源失效。
#[derive(Debug, Clone)]
pub struct ContentToken {
    /// 内容所属固定版本。
    pub snapshot: Arc<Snapshot>,
    /// 完整请求区域。
    pub region: PixelRect,
}
/// 针对一个屏幕来源的完整区域采样请求。
#[derive(Clone)]
pub struct SampleRequest {
    /// 屏幕来源。
    pub source: SourceId,
    /// 屏幕本地物理像素区域。
    pub region: PixelRect,
    /// 所需无变化时长，默认调用方使用 150ms。
    pub quiet: Duration,
    /// 已知内容，用于在读回前精确验证复用。
    pub previous: Option<ContentToken>,
}
/// 是否需要消费者处理新图像。
#[derive(Debug, Clone)]
pub enum SampleContent {
    /// 与 previous 相同，无像素读回。
    Unchanged,
    /// 新的共享像素。
    Image(PixelImage),
}
/// 稳定区域结果，携带来源版本和采集确认水位。
#[derive(Debug, Clone)]
pub struct RegionSample {
    /// 可供下一次内容验证的令牌。
    pub token: ContentToken,
    /// 返回内容。
    pub content: SampleContent,
    /// 本次观察确认的版本。
    pub observed_version: Version,
    /// 健康处理水位。
    pub observed_through: ClockTime,
}
/// 视觉模块只依赖取图契约，不依赖采样服务或 Windows。
pub trait RegionSource: Send + Sync {
    /// 总截止时间包含稳定等待、排队和像素读回。
    fn sample(&self, request: SampleRequest, operation: Operation) -> CaptureFuture<RegionSample>;
}
