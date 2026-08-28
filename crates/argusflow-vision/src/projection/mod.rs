//! 将结构化 VisualScene 投影成 deterministic 文本。

mod compact;
mod spatial;

pub use compact::compact_text;
pub use spatial::spatial_text;

/// 文本投影设置；投影只改变显示，不改变 OCR 事实字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionOptions {
    /// 是否在区域之间输出 `[Header]` 等开发者 marker。
    pub region_markers: bool,
    /// SpatialText 的固定归一化列数。
    pub spatial_columns: usize,
}

impl Default for ProjectionOptions {
    fn default() -> Self {
        Self {
            region_markers: false,
            spatial_columns: 120,
        }
    }
}
