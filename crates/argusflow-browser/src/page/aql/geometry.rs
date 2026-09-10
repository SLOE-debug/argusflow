//! iframe 局部视口到父级视口的有界坐标变换。
use crate::BrowserError;
use argusflow_core::{CssPoint, FailureKind};
use serde::Deserialize;

/// 读取自 iframe 宿主当前布局的轴对齐变换。
#[derive(Debug, Deserialize)]
pub(super) struct FrameMetrics {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub border_left: f64,
    pub border_top: f64,
    pub layout_width: f64,
    pub layout_height: f64,
    pub supported: bool,
}
pub(super) fn project(point: CssPoint, metrics: &FrameMetrics) -> Result<CssPoint, BrowserError> {
    let numbers = [
        metrics.left,
        metrics.top,
        metrics.width,
        metrics.height,
        metrics.border_left,
        metrics.border_top,
        metrics.layout_width,
        metrics.layout_height,
    ];
    if !metrics.supported
        || numbers.iter().any(|value| !value.is_finite())
        || metrics.width <= 0.0
        || metrics.height <= 0.0
        || metrics.layout_width <= 0.0
        || metrics.layout_height <= 0.0
    {
        return Err(BrowserError::new(
            FailureKind::Unsupported,
            "aql_frame_geometry",
            "iframe 无有效布局或包含旋转、倾斜、透视变换",
        ));
    }
    CssPoint::new(
        metrics.left + (metrics.border_left + point.x()) * metrics.width / metrics.layout_width,
        metrics.top + (metrics.border_top + point.y()) * metrics.height / metrics.layout_height,
    )
    .map_err(BrowserError::from)
}
#[cfg(test)]
#[path = "../../../../../tests/argusflow-browser/unit/aql/geometry.rs"]
mod tests;
