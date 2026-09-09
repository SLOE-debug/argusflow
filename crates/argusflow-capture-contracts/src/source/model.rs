//! 时钟和来源版本不以墙钟或窗口句柄代替。
use crate::{PixelRect, Rotation, ScreenRect, SnapshotPixels};
use std::{sync::Arc, time::Duration};

/// 同一采样服务共享的单调时钟域；origin/frequency 描述原始 QPC。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockDomain {
    /// 服务会话身份，进程重启不得复用。
    pub session: u128,
    /// 原始计数器起点。
    pub origin: i64,
    /// 每秒计数数目。
    pub frequency: u64,
}
/// 自时钟域起点起的单调纳秒，不能与其他时钟域直接比较。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockTime(pub u64);
impl ClockTime {
    /// 饱和计算已过去的时间。
    pub fn elapsed_since(self, earlier: Self) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }
    /// 饱和增加时间。
    pub fn after(self, duration: Duration) -> Self {
        Self(
            self.0
                .saturating_add(duration.as_nanos().min(u128::from(u64::MAX)) as u64),
        )
    }
}
/// 会话内稳定的显示输出身份，不等于可复用的 HMONITOR。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub u64);
/// 完整像素版本身份；只有同会话、来源和代际可以比较修订号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    /// 采样会话。
    pub session: u128,
    /// 显示输出。
    pub source: SourceId,
    /// 重建时递增。
    pub generation: u64,
    /// 仅真实变化递增，基线为零。
    pub revision: u64,
}
/// 来源的明确生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    /// 正在建立首个基线。
    Initializing,
    /// 正常采集。
    Ready,
    /// 正在有限重试。
    Recovering,
    /// 重试耗尽或不支持。
    Unavailable,
    /// 输出已移除。
    Removed,
    /// 已释放。
    Stopped,
}
/// 显示器的可见桌面描述。
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// 来源身份。
    pub id: SourceId,
    /// 显示设备名称，不包含窗口或用户输入内容。
    pub name: String,
    /// 来源代际。
    pub generation: u64,
    /// 虚拟桌面物理像素范围。
    pub bounds: ScreenRect,
    /// 原始纹理旋转。
    pub rotation: Rotation,
    /// 有效 DPI，用于检测缩放设置变化；像素坐标仍不进行逻辑缩放。
    pub dpi: (u32, u32),
    /// 生命周期。
    pub state: SourceState,
    /// 最近一次原生失败，恢复成功后清除。
    pub failure: Option<crate::CaptureError>,
}
/// 呈现、冻结和处理进度分开表达。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// 系统呈现时间；初始化无法确定时为空。
    pub presented: Option<ClockTime>,
    /// Acquire 成功后的时间。
    pub acquired: ClockTime,
    /// 图像复制和比较完成时间。
    pub frozen: ClockTime,
}
/// 固定版本的 GPU 或内存图像；storage 只提供安全操作，不暴露原生句柄。
#[derive(Clone)]
pub struct Snapshot {
    /// 完整身份。
    pub version: Version,
    /// 物理像素范围。
    pub bounds: ScreenRect,
    /// 该版本时间信息。
    pub timing: Timing,
    /// 只读图像后端。
    pub pixels: Arc<dyn SnapshotPixels>,
}
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("version", &self.version)
            .field("bounds", &self.bounds)
            .field("timing", &self.timing)
            .finish_non_exhaustive()
    }
}
/// GPU 精确比较结果；矩形可包含内部未变像素，但不漏真实变化。
#[derive(Debug, Clone, Default)]
pub struct PixelChanges {
    /// 图像本地变化区域。
    pub regions: Vec<PixelRect>,
    /// 被比较的不同像素数。
    pub compared_pixels: u64,
    /// 颜色确实改变的像素数。
    pub changed_pixels: u64,
}
