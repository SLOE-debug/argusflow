//! 办公操作的完整桌面帧流；不要求逐次桌面变化的无损历史。
use crate::{
    CaptureFuture, CaptureResult, ClockDomain, ClockTime, PixelImage, SourceInfo, Timing, Version,
};
use argusflow_core::Operation;
use std::sync::Arc;

/// 完整显示器画面，原始屏幕坐标与缩放后图像尺寸分别保存。
#[derive(Debug, Clone)]
pub struct DesktopFrame {
    /// 帧流内身份；revision 随本流接受的画面递增，不表示变化像素计数。
    pub version: Version,
    /// 原始物理屏幕范围、DPI 与代际。
    pub source: SourceInfo,
    /// 原始呈现和实际获取、冻结时间。
    pub timing: Timing,
    /// 按比例缩放后的全屏图像；不可变且有字节预算。
    pub image: PixelImage,
}
/// 一个显示器在此次读取时的健康水位与短历史。
#[derive(Debug, Clone)]
pub struct FrameHistory {
    /// 当前来源描述，故障在 failure 中明确报告。
    pub source: SourceInfo,
    /// 采集已完成检测的时间；冻结中的新画面不会被越过。
    pub checked: ClockTime,
    /// 从旧到新的有限缓存，调用方可用 Arc 固定所需帧。
    pub frames: Vec<Arc<DesktopFrame>>,
}
/// 完整帧来源的生命周期边界；与变化区域采样的消费者独立。
pub trait DesktopFrameSource: Send + Sync {
    /// 本次启动的单调时钟域；恢复录制创建新域。
    fn clock(&self) -> ClockDomain;
    /// 当前单调时间。
    fn now(&self) -> ClockTime;
    /// 只读取缓存引用与元数据，不执行图像编码或新截图。
    fn history(&self) -> CaptureResult<Vec<FrameHistory>>;
    /// 立即请求停止采集，返回时原生工作线程已退出。
    fn shutdown(&self, operation: Operation) -> CaptureFuture<()>;
}

/// 全屏办公采集预算；缩放不改变宽高比例、不放大小屏幕。
#[derive(Debug, Clone)]
pub struct FrameConfig {
    /// 输出最大宽度。
    pub width: u32,
    /// 输出最大高度。
    pub height: u32,
    /// 每个显示器最高采集频率。
    pub fps: u32,
    /// 每个显示器的短历史帧数；总内存仍受 cpu_bytes 限制。
    pub history: usize,
    /// 全部显示器共享的不可变图像预算。
    pub cpu_bytes: usize,
    /// 每适配器的缩放与读回临时资源预算。
    pub gpu_bytes: usize,
}
impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 15,
            history: 8,
            cpu_bytes: 128 * 1024 * 1024,
            gpu_bytes: 256 * 1024 * 1024,
        }
    }
}
impl FrameConfig {
    /// 保持宽高比，将原始尺寸限制在配置范围内。
    pub fn size(&self, width: u32, height: u32) -> (u32, u32) {
        let scale = (f64::from(self.width) / f64::from(width.max(1)))
            .min(f64::from(self.height) / f64::from(height.max(1)))
            .min(1.0);
        (
            ((f64::from(width) * scale).round() as u32).max(1),
            ((f64::from(height) * scale).round() as u32).max(1),
        )
    }
}
