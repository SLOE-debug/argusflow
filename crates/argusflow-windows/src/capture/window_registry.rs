//! 按进程枚举可独立捕获的可见顶层窗口。

use std::mem::size_of;

use argusflow_core::WindowIdentity;
use argusflow_vision::{PhysicalRect, VisionError, WindowDescriptor, WindowInventory};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        UI::WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowThreadProcessId,
            IsWindowVisible,
        },
    },
    core::BOOL,
};

/// Windows 顶层窗口注册表；自身无状态，每次返回一份真实桌面快照。
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowsWindowRegistry;

impl WindowInventory for WindowsWindowRegistry {
    fn windows_for_process(&self, process_id: u32) -> Result<Vec<WindowDescriptor>, VisionError> {
        let mut context = EnumerationContext {
            process_id,
            foreground: unsafe { GetForegroundWindow() },
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(
                Some(enumerate_window),
                LPARAM((&mut context as *mut EnumerationContext).cast::<()>() as isize),
            )
        }
        .map_err(|error| VisionError::CaptureUnavailable {
            message: format!("failed to enumerate process windows: {error}"),
        })?;
        Ok(context.windows)
    }
}

/// EnumWindows 回调拥有的短期收集状态。
struct EnumerationContext {
    /// 只接受此进程的 HWND。
    process_id: u32,
    /// 枚举开始时的前台 HWND。
    foreground: HWND,
    /// EnumWindows 顺序本身即桌面 Z-Order。
    windows: Vec<WindowDescriptor>,
}

/// 将一个满足捕获条件的顶层 HWND 投影成平台无关描述。
unsafe extern "system" fn enumerate_window(window: HWND, state: LPARAM) -> BOOL {
    let context = unsafe { &mut *(state.0 as *mut EnumerationContext) };
    let mut process_id = 0_u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id != context.process_id || !unsafe { IsWindowVisible(window) }.as_bool() {
        return BOOL(1);
    }
    let mut cloaked = 0_u32;
    if unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
    {
        return BOOL(1);
    }
    let Some(bounds) = visible_bounds(window) else {
        return BOOL(1);
    };
    let owner = unsafe { GetWindow(window, GW_OWNER) }
        .ok()
        .filter(|owner| owner.0 != std::ptr::null_mut());
    let z_order = context.windows.len();
    context.windows.push(WindowDescriptor {
        identity: WindowIdentity {
            handle: window.0 as usize as u64,
            process_id,
        },
        owner_handle: owner.map(|owner| owner.0 as usize as u64),
        z_order,
        screen_bounds: bounds,
        foreground: window == context.foreground,
    });
    BOOL(1)
}

/// 读取不含不可见 resize border 的 DWM 物理边界。
fn visible_bounds(window: HWND) -> Option<PhysicalRect> {
    let mut bounds = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut bounds as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()?;
    let width = u32::try_from(bounds.right.checked_sub(bounds.left)?).ok()?;
    let height = u32::try_from(bounds.bottom.checked_sub(bounds.top)?).ok()?;
    PhysicalRect::new(bounds.left, bounds.top, width, height)
}
