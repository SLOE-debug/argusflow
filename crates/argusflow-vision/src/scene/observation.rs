//! VisualScene 的观测完整性与 freshness 快照。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::frame::PhysicalRect;

/// 当前 scene 对所属 viewport 的观测覆盖语义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObservationCoverage {
    /// 尚未成功建立任何视觉事实。
    Empty,
    /// 只观测了列出的区域，不能据此断言全局目标不存在。
    Partial {
        /// 已由成功 OCR 覆盖的区域。
        covered: Vec<PhysicalRect>,
    },
    /// 当前 viewport 已经过完整 bootstrap，后续仅存在显式 dirty freshness。
    Complete,
}

impl ObservationCoverage {
    /// 判断全局 AQL 查询能否安全给出确定否定。
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// 一块成功刷新区域的只读 freshness 摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshRegion {
    /// 帧本地物理区域。
    pub region: PhysicalRect,
    /// 距离该区域最近一次成功 OCR 的毫秒数。
    pub age_ms: u64,
}

/// Planner 和 Evidence 可查询的完整观测状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationState {
    /// viewport 是否已经完整观测。
    pub coverage: ObservationCoverage,
    /// 成功刷新区域及其相对年龄。
    pub fresh_regions: Vec<FreshRegion>,
    /// 尚未被成功 OCR 覆盖的变化区域。
    pub dirty_regions: Vec<PhysicalRect>,
}

impl ObservationState {
    /// 创建尚未观测的初始状态。
    pub const fn empty() -> Self {
        Self {
            coverage: ObservationCoverage::Empty,
            fresh_regions: Vec::new(),
            dirty_regions: Vec::new(),
        }
    }
}

/// 将单调时钟时长安全收窄到协议毫秒。
pub(crate) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
