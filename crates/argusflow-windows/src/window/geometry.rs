//! 窗口范围始终按物理像素读取，不继承调用线程的 DPI 虚拟化。
use crate::{
    WindowIdentity, WindowsError,
    platform::{PhysicalDpi, failure},
};
use argusflow_capture_contracts::ScreenRect;
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
};
impl WindowIdentity {
    /// 读取 DWM 可见物理外框，排除用于调整大小的不可见边框。
    pub fn physical_bounds(&self) -> Result<ScreenRect, WindowsError> {
        self.validate()?;
        let _dpi = PhysicalDpi::enter()?;
        let mut rect = RECT::default();
        // SAFETY: 身份已验证，rect 为有效独占输出；DPI 守卫在返回时恢复。
        unsafe {
            DwmGetWindowAttribute(
                HWND(self.handle() as *mut _),
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut rect as *mut RECT).cast(),
                std::mem::size_of::<RECT>() as u32,
            )
        }
        .map_err(|e| failure("window_bounds", e))?;
        ScreenRect::new(
            rect.left,
            rect.top,
            (i64::from(rect.right) - i64::from(rect.left))
                .try_into()
                .map_err(|_| {
                    WindowsError::new(
                        argusflow_core::FailureKind::InvalidInput,
                        "window_bounds",
                        "窗口宽度无效",
                    )
                })?,
            (i64::from(rect.bottom) - i64::from(rect.top))
                .try_into()
                .map_err(|_| {
                    WindowsError::new(
                        argusflow_core::FailureKind::InvalidInput,
                        "window_bounds",
                        "窗口高度无效",
                    )
                })?,
        )
        .map_err(|e| match e {
            argusflow_capture_contracts::CaptureError::Failure(e) => e.into(),
        })
    }
}
