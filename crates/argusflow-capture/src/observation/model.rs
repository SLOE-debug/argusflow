//! 观察结果显式区分稳定、超时、缺口和不可用。
use argusflow_capture_contracts::*;
use std::sync::Arc;

/// 有期限的交互前版本租约。过期不再允许启动读取。
#[derive(Clone)]
pub struct Anchor {
    pub(crate) snapshot: Arc<Snapshot>,
    pub(crate) at: ClockTime,
    pub(crate) expires: ClockTime,
}
impl Anchor {
    /// 被证明在输入时间前存在的版本。
    pub fn version(&self) -> Version {
        self.snapshot.version
    }
    /// 输入所用时间锚点。
    pub fn at(&self) -> ClockTime {
        self.at
    }
    /// 租约期限。
    pub fn expires(&self) -> ClockTime {
        self.expires
    }
}
/// 实际观察到的过程摘要，不把系统合并帧数当变化次数。
#[derive(Debug, Clone, Default)]
pub struct ProcessSummary {
    /// 目标范围内观察到的真实变化版本数。
    pub changes: u64,
    /// 期间变化区域的并集。
    pub regions: Vec<PixelRect>,
    /// 最后一次真实变化时间。
    pub last_change: ClockTime,
    /// 期间所有已知缺口。
    pub gaps: Vec<Gap>,
}
/// 一次观察的终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationStatus {
    /// 稳定且相对起点有净变化。
    StableChanged,
    /// 稳定且最终无净变化，过程仍可能有变化。
    StableUnchanged,
    /// 到达时限仍未稳定。
    TimedOutUnstable,
    /// 历史不足或不确定。
    HistoryGap,
    /// 来源已不可用。
    SourceUnavailable,
    /// 请求被取消。
    Cancelled,
}
/// 稳定图像按读取区域独立交付，无完整屏幕视频帧。
pub struct Observation {
    /// 明确终态。
    pub status: ObservationStatus,
    /// 起点版本。
    pub before: Version,
    /// 仅已确认的终点版本。
    pub after: Option<Version>,
    /// 观察处理水位。
    pub through: ClockTime,
    /// 过程摘要。
    pub process: ProcessSummary,
    /// 精确比较得到的净变化范围。
    pub exact_regions: Vec<PixelRect>,
    /// 扩展后的读取区域及共享像素。
    pub images: Vec<(PixelRect, PixelImage)>,
}
