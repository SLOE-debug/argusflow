//! 保持等比例坐标，不通过分别归一化横纵坐标改变角度。
use argusflow_core::{Failure, FailureKind};
use serde::Serialize;

/// 同一坐标空间中的 [x, y, width, height]。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Rect(pub(super) [f64; 4]);
impl Rect {
    /// 返回只读外框坐标副本：[x, y, width, height]。
    pub fn coordinates(self) -> [f64; 4] {
        self.0
    }
    /// 拒绝非有限、空或超过预算的几何。
    pub fn new(bounds: [f64; 4]) -> Result<Self, Failure> {
        let [x, y, w, h] = bounds;
        if bounds.iter().any(|v| !v.is_finite() || v.abs() > 1e12)
            || w <= 0.0
            || h <= 0.0
            || !(x + w).is_finite()
            || !(y + h).is_finite()
        {
            return Err(Failure::new(
                FailureKind::Unsupported,
                "aql_geometry",
                "来源没有可靠的非空目标边界",
            ));
        }
        Ok(Self(bounds))
    }
    /// 外框中心。
    pub fn center(self) -> [f64; 2] {
        [self.0[0] + self.0[2] / 2.0, self.0[1] + self.0[3] / 2.0]
    }
    /// 是否完全包含另一外框。
    pub fn contains(self, other: Self) -> bool {
        other.0[0] >= self.0[0]
            && other.0[1] >= self.0[1]
            && other.0[0] + other.0[2] <= self.0[0] + self.0[2]
            && other.0[1] + other.0[3] <= self.0[1] + self.0[3]
    }
    /// 是否有正面积交集。
    pub fn overlaps(self, other: Self) -> bool {
        self.0[0] < other.0[0] + other.0[2]
            && other.0[0] < self.0[0] + self.0[2]
            && self.0[1] < other.0[1] + other.0[3]
            && other.0[1] < self.0[1] + self.0[3]
    }
    /// 两外框最短距离。
    pub fn edge_distance(self, other: Self) -> f64 {
        let dx = (self.0[0] - other.0[0] - other.0[2])
            .max(other.0[0] - self.0[0] - self.0[2])
            .max(0.0);
        let dy = (self.0[1] - other.0[1] - other.0[3])
            .max(other.0[1] - self.0[1] - self.0[3])
            .max(0.0);
        dx.hypot(dy)
    }
}
/// 空间身份由适配器确认；无法确认 DPI 时只允许范围百分比长度。
#[derive(Debug, Clone)]
pub struct Geometry {
    /// 目标外框。
    pub(super) bounds: Rect,
    /// 已确认的查找范围。
    pub(super) scope: Rect,
    /// 窗口、frame 或图像身份。
    pub(super) space: String,
    /// 每个逻辑像素对应的当前坐标单位。
    pub(super) pixels_per_logical: Option<f64>,
}
impl Geometry {
    /// 拒绝非有限、空或超过预算的几何。
    pub fn new(
        bounds: Rect,
        scope: Rect,
        space: impl Into<String>,
        pixels_per_logical: Option<f64>,
    ) -> Result<Self, Failure> {
        let space = space.into();
        Rect::new(bounds.0)?;
        Rect::new(scope.0)?;
        if space.is_empty() || pixels_per_logical.is_some_and(|v| !v.is_finite() || v <= 0.0) {
            return Err(Failure::new(
                FailureKind::Unsupported,
                "aql_geometry",
                "坐标空间或等比例单位未经确认",
            ));
        }
        Ok(Self {
            bounds,
            scope,
            space,
            pixels_per_logical,
        })
    }
}
