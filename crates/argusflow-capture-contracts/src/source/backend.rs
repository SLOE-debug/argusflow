//! 平台后端管理原生线程，服务不持有或操作原生设备。
use crate::{
    CaptureResult, ClockDomain, ClockTime, Gap, PixelChanges, PixelImage, PixelRect, Snapshot,
    SourceId, SourceInfo,
};
use argusflow_core::Operation;
use std::{any::Any, future::Future, pin::Pin, sync::Arc, time::Duration};

/// 不绑定执行器的可发送异步操作。
pub type CaptureFuture<T> = Pin<Box<dyn Future<Output = CaptureResult<T>> + Send + 'static>>;

/// 不可变像素的真实平台边界；比较和读取必须遵守同一操作票据。
pub trait SnapshotPixels: Any + Send + Sync {
    /// 仅供所属平台验证同一实现，不向调用方泄漏原生类型。
    fn as_any(&self) -> &dyn Any;
    /// 按需读取单个区域，不修改快照。
    fn read(self: Arc<Self>, region: PixelRect, operation: Operation) -> CaptureFuture<PixelImage>;
    /// 比较两个同来源、同代际快照的指定区域，不读回像素。
    fn compare(
        self: Arc<Self>,
        before: Arc<dyn SnapshotPixels>,
        regions: Vec<PixelRect>,
        operation: Operation,
    ) -> CaptureFuture<PixelChanges>;
    /// 来源是否仍有效，设备重建立即撤销旧代际。
    fn valid(&self) -> bool;
}
/// 原生后台向共享服务交付的顺序事件。
#[derive(Debug, Clone)]
pub enum BackendEvent {
    /// 来源新增或生命周期变化。
    Source(SourceInfo),
    /// 初始化或重建基线，不算真实变化。
    Baseline(Arc<Snapshot>),
    /// 真实像素变化和固定版本。
    Changed {
        /// 更新后的快照。
        snapshot: Arc<Snapshot>,
        /// 精确比较摘要。
        changes: PixelChanges,
    },
    /// 健康采集已排除此前所有待处理更新。
    Watermark {
        /// 屏幕来源。
        source: SourceId,
        /// 来源代际。
        generation: u64,
        /// 完成检测的单调时间。
        through: ClockTime,
    },
    /// 不完整历史，必须先于受影响的后续版本交付。
    Gap(Gap),
}
/// 原生预算及生命周期设置。
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// 每适配器自有 GPU 字节预算。
    pub gpu_bytes: usize,
    /// 所有适配器共享的 CPU 图像预算。
    pub cpu_bytes: usize,
    /// 每适配器在途帧上限。
    pub max_in_flight: usize,
    /// 单次原生等待上限。
    pub acquire_wait: Duration,
    /// GPU 操作完成时限。
    pub gpu_timeout: Duration,
    /// 一轮设备恢复总时限。
    pub recovery_timeout: Duration,
    /// 原生事件队列字节预算。
    pub event_bytes: usize,
    /// 原生读取等待队列容量。
    pub read_queue: usize,
}
impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            gpu_bytes: 256 * 1024 * 1024,
            cpu_bytes: 128 * 1024 * 1024,
            max_in_flight: 3,
            acquire_wait: Duration::from_millis(8),
            gpu_timeout: Duration::from_millis(500),
            recovery_timeout: Duration::from_secs(10),
            event_bytes: 4 * 1024 * 1024,
            read_queue: 32,
        }
    }
}
/// 只包含工作量和资源占用的诊断。
#[derive(Debug, Clone, Default)]
pub struct CaptureStats {
    /// 成功获取的原生帧数，包括独立光标。
    pub acquired_frames: u64,
    /// 真实变化发布数。
    pub changed_frames: u64,
    /// GPU 向 CPU 读回的元数据字节。
    pub metadata_readback_bytes: u64,
    /// GPU 向 CPU 读回的图像字节。
    pub pixel_readback_bytes: u64,
    /// 发布的缺口数。
    pub gaps: u64,
    /// GPU 自有资源当前字节数。
    pub gpu_bytes: usize,
    /// GPU 自有资源峰值总计。
    pub gpu_peak_bytes: usize,
    /// CPU 图像当前字节数。
    pub cpu_bytes: usize,
    /// CPU 图像峰值。
    pub cpu_peak_bytes: usize,
}
/// 显式启动的共享后台。Clone 应共享实例；poll 只有一个服务消费者。
pub trait DesktopBackend: Send + Sync {
    /// 排他占用事件消费端，防止两个共享服务互相取走事件。
    fn claim_consumer(&self) -> CaptureResult<()>;
    /// 服务销毁后释放消费端占用；不关闭原生来源。
    fn release_consumer(&self);
    /// 时钟域。
    fn clock(&self) -> ClockDomain;
    /// 当前单调时间。
    fn now(&self) -> ClockTime;
    /// 非阻塞取走一批事件，返回顺序不得变化。
    fn poll(&self) -> CaptureResult<Vec<BackendEvent>>;
    /// 不包含图像的统计快照。
    fn stats(&self) -> CaptureStats;
    /// 唤醒一次显式恢复；不自动无限重试。
    fn restart(&self) -> CaptureResult<()>;
    /// 停止工作并在截止时间内回收原生线程。
    fn shutdown(&self, operation: Operation) -> CaptureFuture<()>;
}
