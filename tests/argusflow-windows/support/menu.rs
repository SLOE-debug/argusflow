//! 在测试窗口线程中运行系统菜单，记录真实 WM_COMMAND 选择结果。
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::*,
    },
    core::w,
};
pub const OPEN: u32 = WM_APP + 19;
pub static MENU: AtomicIsize = AtomicIsize::new(0);
pub static SUBMENU: AtomicIsize = AtomicIsize::new(0);
pub static SELECTED: AtomicUsize = AtomicUsize::new(0);
pub unsafe fn show(window: HWND) {
    let _dpi = super::DpiContext::physical();
    unsafe {
        SELECTED.store(0, Ordering::SeqCst);
        let menu = CreatePopupMenu().unwrap();
        let submenu = CreatePopupMenu().unwrap();
        AppendMenuW(menu, MF_STRING, 901, w!("New document")).unwrap();
        AppendMenuW(submenu, MF_STRING, 902, w!("Nested action")).unwrap();
        AppendMenuW(menu, MF_POPUP, submenu.0 as usize, w!("More actions")).unwrap();
        let mut bounds = RECT::default();
        GetWindowRect(window, &mut bounds).unwrap();
        MENU.store(menu.0 as isize, Ordering::SeqCst);
        SUBMENU.store(submenu.0 as isize, Ordering::SeqCst);
        let result = TrackPopupMenu(
            menu,
            TPM_RETURNCMD,
            bounds.right + 20,
            bounds.top + 40,
            None,
            window,
            None,
        );
        SELECTED.store(result.0 as usize, Ordering::SeqCst);
        MENU.store(0, Ordering::SeqCst);
        SUBMENU.store(0, Ordering::SeqCst);
        DestroyMenu(menu).unwrap();
    }
}
