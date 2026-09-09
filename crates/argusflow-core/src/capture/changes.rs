//! 与冻结像素绑定的有界变化历史，不持有历史像素。
use super::frame::{FrameId, PhysicalRect};
use std::sync::Arc;

/// 相邻两个同来源、同拓扑版本之间的精确变化范围。
#[derive(Debug, Clone)]
pub struct FrameChange {
    /// 差分基准身份。
    pub after: FrameId,
    /// 变化完成时的帧身份。
    pub through: FrameId,
    /// 帧内物理矩形；为空表示像素没有变化。
    pub regions: Arc<[PhysicalRect]>,
}

/// 返回指定基准后的所有变化；历史不连续时要求调用方完整刷新。
pub fn changes_since(
    history: &[FrameChange],
    previous: FrameId,
    current: FrameId,
) -> Option<Vec<PhysicalRect>> {
    if previous == current {
        return Some(Vec::new());
    }
    let start = history.iter().position(|change| change.after == previous)?;
    let mut cursor = previous;
    let mut regions = Vec::new();
    for change in &history[start..] {
        if change.after != cursor {
            return None;
        }
        regions.extend_from_slice(&change.regions);
        cursor = change.through;
        if cursor == current {
            return Some(regions);
        }
    }
    None
}
