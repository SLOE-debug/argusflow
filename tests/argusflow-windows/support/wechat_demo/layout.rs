//! 微信 demo 的布局范围，只用于筛选全窗口 OCR 与无文字输入区定位。
use argusflow_capture_contracts::{CaptureError, PixelRect, ScreenRect};

/// 默认双栏微信的语义区域，不参与 OCR 差分规划。
#[derive(Clone, Copy)]
pub enum Zone {
    /// 搜索框与会话标题。
    Header,
    /// 左侧会话列表。
    Sidebar,
    /// 输入框，排除工具栏。
    Editor,
    /// 会话内容。
    Messages,
}
impl Zone {
    /// 当前窗口局部坐标，窗口大小变化后重新计算。
    pub fn region(self, bounds: ScreenRect) -> Result<PixelRect, CaptureError> {
        let (left, top, right, bottom) = match self {
            Self::Header => (0.0, 0.03, 1.0, 0.13),
            Self::Sidebar => (0.07, 0.13, 0.34, 0.95),
            Self::Editor => (0.35, 0.765, 0.98, 0.91),
            Self::Messages => (0.35, 0.13, 0.99, 0.76),
        };
        PixelRect::new(
            (bounds.width() as f64 * left) as u32,
            (bounds.height() as f64 * top) as u32,
            (bounds.width() as f64 * (right - left)) as u32,
            (bounds.height() as f64 * (bottom - top)) as u32,
        )
    }
}
