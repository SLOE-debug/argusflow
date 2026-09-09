use super::*;

impl Stamp {
    pub(crate) fn test_stamp() -> Arc<Self> {
        Arc::new(Self {
            handle: 0,
            token: 0,
        })
    }
}
use windows::{
    Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    },
    core::w,
};
#[test]
fn destroyed_window_and_removed_property_invalidate_identity_without_rebinding() {
    // 无 UIA / 输入操作；仅建立本测试拥有的不可见 STATIC HWND 验证身份租约。
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("identity fixture"),
            WINDOW_STYLE::default(),
            0,
            0,
            1,
            1,
            None,
            None,
            None,
            None,
        )
    }
    .unwrap();
    let identity = crate::WindowIdentity::from_handle(window.0 as isize).unwrap();
    identity.validate().unwrap();
    unsafe { RemovePropW(window, &name()) }.unwrap();
    assert_eq!(
        identity.validate().unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    let renewed = crate::WindowIdentity::from_handle(window.0 as isize).unwrap();
    assert_ne!(identity, renewed);
    unsafe { DestroyWindow(window) }.unwrap();
    assert_eq!(
        renewed.validate().unwrap_err().kind(),
        FailureKind::StaleHandle
    );
}
