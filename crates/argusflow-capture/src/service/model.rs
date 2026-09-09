//! 日志只保留元数据，不因订阅历史固定图像资源。
use argusflow_capture_contracts::{ClockTime, Gap, PixelChanges, SourceId, SourceInfo, Version};

/// 共享日志中的事件种类。
#[derive(Debug, Clone)]
pub enum ChangeKind {
    /// 来源状态变更。
    Source(SourceInfo),
    /// 新代际基线。
    Baseline(Version),
    /// 真实像素变化。
    Pixels {
        /// 更新版本。
        version: Version,
        /// 变化摘要。
        changes: PixelChanges,
    },
    /// 明确缺口。
    Gap(Gap),
}
/// 递增序号的追加式变化元数据。
#[derive(Debug, Clone)]
pub struct ChangeRecord {
    /// 服务内严格递增序号。
    pub sequence: u64,
    /// 屏幕来源。
    pub source: SourceId,
    /// 事件呈现或观察时间。
    pub time: ClockTime,
    /// 记录内容。
    pub kind: ChangeKind,
}
impl ChangeRecord {
    pub(super) fn byte_len(&self) -> usize {
        256 + match &self.kind {
            ChangeKind::Pixels { changes, .. } => {
                changes.regions.capacity()
                    * std::mem::size_of::<argusflow_capture_contracts::PixelRect>()
            }
            ChangeKind::Source(info) => info.name.capacity(),
            _ => 0,
        }
    }
}
