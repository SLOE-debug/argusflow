//! Windows 窗口 DPI 查询；视觉坐标始终以物理像素保存。

use windows::Win32::{Foundation::HWND, UI::HiDpi::GetDpiForWindow};

/// 读取窗口当前 DPI；系统返回零时使用物理像素契约的最小有效值。
pub(super) fn window_dpi(hwnd: HWND) -> u32 {
    // SAFETY: hwnd 已由调用方完成 IsWindow 和进程身份复验。
    unsafe { GetDpiForWindow(hwnd).max(1) }
}
