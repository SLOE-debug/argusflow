//! OCR 等消费者所需的最小区域采样接口。
use crate::{CaptureFuture, ClockTime, PixelImage, PixelRect, Snapshot, SourceId, Version};
use argusflow_core::Operation;
use std::{sync::Arc, time::Duration};

/// 一份已确认区域内容的比较令牌，持有图像资源直到令牌释放。
#[derive(Debug, Clone)]
pub struct ContentToken {
    /// 内容所属固定版本。
    pub snapshot: Arc<Snapshot>,
    /// 完整请求区域。
    pub region: PixelRect,
    /// 本次区域的原始分辨率共享像素，用于后续精确内容比较。
    pub image: PixelImage,
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
    /// 已知内容，用于精确验证区域内容复用。
    pub previous: Option<ContentToken>,
}
/// 是否需要消费者处理新图像。
#[derive(Debug, Clone)]
pub enum SampleContent {
    /// 与 previous 内容相同，消费者无需再次处理图像。
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
    /// 总截止时间包含稳定等待与像素处理。
    fn sample(&self, request: SampleRequest, operation: Operation) -> CaptureFuture<RegionSample>;
}
