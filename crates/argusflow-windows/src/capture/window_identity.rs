//! HWND 与跨平台窗口身份之间的 Windows 边界校验。

use std::ffi::c_void;

use argusflow_core::WindowIdentity;
use argusflow_vision::VisionError;
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
};

/// 把领域层的不透明 HWND 表示恢复为 Windows 类型。
pub(super) fn native_window(handle: u64) -> HWND {
    HWND(handle as usize as *mut c_void)
}

/// 只接受仍指向原 PID 的 HWND，防止句柄复用导致跨应用捕获。
pub(super) fn validate_window(hwnd: HWND, expected: WindowIdentity) -> Result<(), VisionError> {
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(VisionError::WindowIdentityChanged {
            expected,
            actual: None,
        });
    }
    let mut process_id = 0_u32;
    // SAFETY: process_id 是同步 Win32 调用的独占输出，hwnd 已通过 IsWindow 校验。
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id != expected.process_id {
        return Err(VisionError::WindowIdentityChanged {
            expected,
            actual: Some(WindowIdentity {
                handle: expected.handle,
                process_id,
            }),
        });
    }
    Ok(())
}
