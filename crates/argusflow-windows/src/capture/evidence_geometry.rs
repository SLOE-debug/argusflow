//! 截图前后的最小窗口身份和几何复验，不重复枚举子窗口或读取进程路径。

use argusflow_core::{InspectionContext, InspectionFailure, InspectionRect};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::*,
};

/// 只验证截图使用的 HWND/PID、可见性与物理边界，不重建完整 InspectionContext。
pub(super) fn validate(context: &InspectionContext) -> Result<(), InspectionFailure> {
    let window = HWND(context.window.handle as *mut _);
    let mut process_id = 0;
    let mut rect = RECT::default();
    // SAFETY: 输出均为本地栈值，句柄仅查询；无效 HWND 不解引用。
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut process_id));
        if process_id != context.window.process_id
            || !IsWindowVisible(window).as_bool()
            || IsIconic(window).as_bool()
        {
            return Err(InspectionFailure::ContextChanged);
        }
        GetWindowRect(window, &mut rect).map_err(|_| InspectionFailure::ContextChanged)?;
    }
    let bounds = InspectionRect {
        x: rect.left.into(),
        y: rect.top.into(),
        width: f64::from(rect.right) - f64::from(rect.left),
        height: f64::from(rect.bottom) - f64::from(rect.top),
    };
    if bounds != context.bounds {
        return Err(InspectionFailure::ContextChanged);
    }
    Ok(())
}

/// 将窗口区域裁到虚拟屏幕内；调用线程必须处于物理 DPI 上下文。
pub(super) fn visible_bounds(bounds: InspectionRect) -> Result<InspectionRect, InspectionFailure> {
    // SAFETY: 系统虚拟屏幕指标是只读查询。
    let (x, y, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let left = bounds.x.max(f64::from(x));
    let top = bounds.y.max(f64::from(y));
    let right = (bounds.x + bounds.width).min(f64::from(x) + f64::from(width));
    let bottom = (bounds.y + bounds.height).min(f64::from(y) + f64::from(height));
    let clipped = InspectionRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    if !clipped.is_valid() || clipped.width * clipped.height > 32_000_000.0 {
        return Err(InspectionFailure::InvalidGeometry);
    }
    Ok(clipped)
}
