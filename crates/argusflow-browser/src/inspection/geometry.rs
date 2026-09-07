//! 以原生 renderer client 原点与页面 DPR 校准坐标，支持负屏幕坐标和浏览器缩放。

use argusflow_core::{InspectionContext, InspectionFailure, InspectionRect, ScreenPoint};
use serde::Deserialize;

/// 固定只读脚本，既不读取输入值，也不把 URL query/hash 发回宿主。
pub(super) const METRICS_SCRIPT: &str = r#"(() => ({
    width: innerWidth, height: innerHeight, dpr: devicePixelRatio,
    scale: visualViewport?.scale ?? 1,
    offset_x: visualViewport?.offsetLeft ?? 0, offset_y: visualViewport?.offsetTop ?? 0,
    focused: document.hasFocus(), visible: document.visibilityState === 'visible',
    url: location.origin + location.pathname
}))()"#;

/// 浏览器 CSS viewport 与显示状态；全部在同一次页面求值中采集。
#[derive(Deserialize)]
pub(super) struct ViewportMetrics {
    /// 布局 viewport CSS 宽度（含滚动条）。
    pub width: f64,
    /// 布局 viewport CSS 高度。
    pub height: f64,
    /// 包含显示器 DPI 与浏览器 page zoom 的 physical/CSS 比率。
    pub dpr: f64,
    /// 触摸 pinch zoom，不等于浏览器 page zoom。
    pub scale: f64,
    /// visual viewport 相对 layout viewport 偏移。
    pub offset_x: f64,
    /// visual viewport 垂直偏移。
    pub offset_y: f64,
    /// 页面焦点用于区分同一 browser 进程中的标签/窗口。
    pub focused: bool,
    /// 后台页不允许坐标录制。
    pub visible: bool,
    /// 不含 query/hash/userinfo 的地址。
    pub url: String,
}

/// 通过验证的 physical screen ↔ CSS viewport 变换。
pub(super) struct ViewportTransform {
    /// 原生 renderer client 范围，屏幕物理单位。
    viewport: InspectionRect,
    /// 每 CSS 像素对应的物理像素；不缩放屏幕原点。
    dpr: f64,
}

impl ViewportTransform {
    pub(super) fn new(
        context: &InspectionContext,
        metrics: &ViewportMetrics,
    ) -> Result<Self, InspectionFailure> {
        if !metrics.focused || !metrics.visible {
            return Err(InspectionFailure::ContextChanged);
        }
        let viewport = context
            .browser_viewport
            .ok_or(InspectionFailure::InvalidGeometry)?;
        // 第一阶段拒绝 pinch/device emulation；普通 page zoom 由 DPR 自动覆盖。
        if !viewport.is_valid()
            || !metrics.dpr.is_finite()
            || metrics.dpr <= 0.0
            || !metrics.width.is_finite()
            || !metrics.height.is_finite()
            || metrics.width <= 0.0
            || metrics.height <= 0.0
            || metrics.scale != 1.0
            || metrics.offset_x != 0.0
            || metrics.offset_y != 0.0
            || (viewport.width - metrics.width * metrics.dpr).abs() > 3.0
            || (viewport.height - metrics.height * metrics.dpr).abs() > 3.0
        {
            return Err(InspectionFailure::InvalidGeometry);
        }
        Ok(Self {
            viewport,
            dpr: metrics.dpr,
        })
    }

    /// CDP 坐标相对 viewport，不能再加 document.scrollX/Y。
    pub(super) fn to_css(&self, point: ScreenPoint) -> Result<(i32, i32), InspectionFailure> {
        if !self.viewport.contains(point) {
            return Err(InspectionFailure::InvalidGeometry);
        }
        Ok((
            ((f64::from(point.x) - self.viewport.x) / self.dpr).floor() as i32,
            ((f64::from(point.y) - self.viewport.y) / self.dpr).floor() as i32,
        ))
    }

    /// DOM getBoundingClientRect 的 CSS 几何转回屏幕 physical，供 Trace 消费。
    pub(super) fn screen_bounds(&self, rect: InspectionRect) -> InspectionRect {
        InspectionRect {
            x: self.viewport.x + rect.x * self.dpr,
            y: self.viewport.y + rect.y * self.dpr,
            width: rect.width * self.dpr,
            height: rect.height * self.dpr,
        }
    }
}
