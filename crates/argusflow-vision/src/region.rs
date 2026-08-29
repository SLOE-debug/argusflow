//! 视觉查询区域在归一化视口与物理像素之间的转换。

use argusflow_core::NormalizedRect;

use crate::frame::PhysicalRect;

/// 将合法的归一化区域映射到当前视觉视口的物理像素矩形。
///
/// 归一化区域只描述查询范围，不携带屏幕坐标；所有调用方都必须以当次 scene/frame
/// 的 viewport 进行换算，避免窗口缩放或 DPI 变化后复用固定坐标。
pub fn normalized_region_to_physical(
    region: NormalizedRect,
    viewport: PhysicalRect,
) -> Option<PhysicalRect> {
    if !region.is_valid() {
        return None;
    }
    let left = (f64::from(viewport.x) + f64::from(region.x()) * f64::from(viewport.width)).floor();
    let top = (f64::from(viewport.y) + f64::from(region.y()) * f64::from(viewport.height)).floor();
    let right = (f64::from(viewport.x)
        + f64::from(region.x() + region.width()) * f64::from(viewport.width))
    .ceil();
    let bottom = (f64::from(viewport.y)
        + f64::from(region.y() + region.height()) * f64::from(viewport.height))
    .ceil();
    if !left.is_finite()
        || !top.is_finite()
        || !right.is_finite()
        || !bottom.is_finite()
        || left < f64::from(i32::MIN)
        || top < f64::from(i32::MIN)
        || right > f64::from(i32::MAX)
        || bottom > f64::from(i32::MAX)
    {
        return None;
    }
    PhysicalRect::new(
        left as i32,
        top as i32,
        (right - left).max(1.0) as u32,
        (bottom - top).max(1.0) as u32,
    )
}
