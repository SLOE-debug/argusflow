//! 枚举主窗口和已确认归属的可见顶层表面，为 OCR 提供精确区域集合。
use crate::{
    WindowIdentity, WindowsError,
    platform::{PhysicalDpi, failure},
};
use argusflow_capture_contracts::ScreenRect;
use argusflow_core::FailureKind;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::*,
    },
    core::BOOL,
};

impl WindowIdentity {
    /// 当前主窗口、owned 窗口及活动菜单的物理范围；不包括同进程无关窗口。
    pub fn physical_surfaces(&self) -> Result<Vec<ScreenRect>, WindowsError> {
        self.validate()?;
        let _dpi = PhysicalDpi::enter()?;
        let mut handles: Vec<HWND> = Vec::new();
        // SAFETY: EnumWindows 同步回调；指针只在调用期间访问，回调不保存指针。
        unsafe {
            EnumWindows(
                Some(collect),
                LPARAM((&mut handles as *mut Vec<HWND>) as isize),
            )
        }
        .map_err(|e| failure("window_surfaces", e))?;
        let mut regions = vec![self.physical_bounds()?];
        for handle in handles {
            if handle.0 as isize == self.handle() || !self.owns_surface(handle)? {
                continue;
            }
            let mut rect = RECT::default();
            // SAFETY: 可见性与窗口范围均为同步只读查询；窗口销毁的错误明确传播。
            if !unsafe { IsWindowVisible(handle) }.as_bool() {
                continue;
            }
            unsafe { GetWindowRect(handle, &mut rect) }
                .map_err(|e| failure("window_surface_bounds", e))?;
            if rect.right > rect.left && rect.bottom > rect.top {
                regions.push(
                    ScreenRect::new(
                        rect.left,
                        rect.top,
                        (i64::from(rect.right) - i64::from(rect.left)) as u32,
                        (i64::from(rect.bottom) - i64::from(rect.top)) as u32,
                    )
                    .map_err(|e| {
                        WindowsError::new(
                            FailureKind::InvalidInput,
                            "window_surface_bounds",
                            e.to_string(),
                        )
                    })?,
                );
            }
        }
        self.validate()?;
        Ok(regions)
    }
    /// 包围所有已确认表面的范围；消费者仍应按 physical_surfaces 排除矩形间空隙。
    pub fn interaction_bounds(&self) -> Result<ScreenRect, WindowsError> {
        let regions = self.physical_surfaces()?;
        let left = regions.iter().map(|r| r.x()).min().unwrap_or(0);
        let top = regions.iter().map(|r| r.y()).min().unwrap_or(0);
        let right = regions
            .iter()
            .map(|r| i64::from(r.x()) + i64::from(r.width()))
            .max()
            .unwrap_or(0);
        let bottom = regions
            .iter()
            .map(|r| i64::from(r.y()) + i64::from(r.height()))
            .max()
            .unwrap_or(0);
        ScreenRect::new(
            left,
            top,
            (right - i64::from(left)) as u32,
            (bottom - i64::from(top)) as u32,
        )
        .map_err(|e| WindowsError::new(FailureKind::InvalidInput, "window_surfaces", e.to_string()))
    }
}
unsafe extern "system" fn collect(handle: HWND, state: LPARAM) -> BOOL {
    // SAFETY: 指针由上方同步 EnumWindows 调用传入，类型与生命周期固定。
    let handles = unsafe { &mut *(state.0 as *mut Vec<HWND>) };
    handles.push(handle);
    BOOL(1)
}
