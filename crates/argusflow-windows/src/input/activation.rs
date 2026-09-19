//! 窗口激活统一使用鼠标移动和 SetForegroundWindow，不点击、不改变置顶状态。
use super::{inject, submission};
use crate::{
    WindowIdentity, WindowsError,
    platform::{PhysicalDpi, failure, hwnd},
};
use argusflow_core::{FailureKind, Operation, ScreenPoint};
use std::time::{Duration, Instant};
use windows::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};

pub(crate) fn activate(window: &WindowIdentity, operation: &Operation) -> Result<(), WindowsError> {
    window.validate()?;
    operation.check("window_activate")?;
    let handle = hwnd(window.handle());
    // 隐藏/最小化后，前台 HWND 可能尚未切换；不能仅凭句柄提前返回。
    if window.require_foreground().is_ok()
        && unsafe { IsWindowVisible(handle) }.as_bool()
        && !unsafe { IsIconic(handle) }.as_bool()
    {
        return Ok(());
    }
    let _dpi = PhysicalDpi::enter()?;
    // SAFETY: 身份已校验；异步恢复避免等待目标窗口的消息线程。
    let minimized = unsafe { IsIconic(handle) }.as_bool();
    if minimized || !unsafe { IsWindowVisible(handle) }.as_bool() {
        operation.begin_effect("window_restore")?;
        unsafe { ShowWindowAsync(handle, if minimized { SW_RESTORE } else { SW_SHOW }) }
            .ok()
            .map_err(|e| failure("window_restore", e))?;
        let deadline = Instant::now() + Duration::from_millis(500);
        while unsafe { IsIconic(handle) }.as_bool() || !unsafe { IsWindowVisible(handle) }.as_bool()
        {
            operation.check("window_restore_wait")?;
            window.validate()?;
            if Instant::now() >= deadline {
                return Err(WindowsError::new(
                    FailureKind::Unresponsive,
                    "window_restore",
                    "窗口未在时限内恢复",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    if window.require_foreground().is_err() {
        // 拒绝用户正在拖拽或按修饰键的状态，不释放用户按键。
        for key in [
            VK_LBUTTON,
            VK_RBUTTON,
            VK_MBUTTON,
            VK_XBUTTON1,
            VK_XBUTTON2,
            VK_MENU,
            VK_CONTROL,
            VK_SHIFT,
            VK_LWIN,
            VK_RWIN,
        ] {
            inject::ensure_released(key)?;
        }
        // SAFETY: 当前 DPI 上下文为物理像素，包含多显示器虚拟桌面的负原点。
        let (left, top, height) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        let point = ScreenPoint {
            x: left,
            y: top.saturating_add(500.min(height.saturating_sub(1))),
        };
        let event = inject::absolute_move(point)?;
        window.validate()?;
        operation.begin_effect("window_activation_move")?;
        submission::submit(&[event])?;
        operation.check("window_activation_request")?;
        window.validate()?;
        // SAFETY: 仅请求激活已验证窗口；BOOL 不代表最终前台状态，统一在下方复验。
        let _ = unsafe { SetForegroundWindow(handle) };
    }
    let deadline = Instant::now() + Duration::from_millis(700);
    let mut stable_since = None;
    loop {
        operation.check("window_activation_wait")?;
        window.validate()?;
        if window.require_foreground().is_ok() {
            let since = stable_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_millis(150) {
                return Ok(());
            }
        } else {
            stable_since = None;
        }
        if Instant::now() >= deadline {
            return Err(WindowsError::new(
                FailureKind::Unavailable,
                "window_activate",
                "窗口未能稳定获得前台",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
