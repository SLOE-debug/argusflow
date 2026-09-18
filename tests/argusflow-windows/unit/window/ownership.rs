use super::*;
use windows::core::w;

#[test]
fn ownership_requires_explicit_owner_chain_not_shared_process() {
    // 不显示窗口，不抢焦点；句柄全部由当前线程创建和释放。
    unsafe {
        let main = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("main"),
            WS_OVERLAPPED,
            0,
            0,
            100,
            100,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let popup = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("owned"),
            WS_POPUP,
            0,
            0,
            100,
            100,
            Some(main),
            None,
            None,
            None,
        )
        .unwrap();
        let nested = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("nested"),
            WS_POPUP,
            0,
            0,
            100,
            100,
            Some(popup),
            None,
            None,
            None,
        )
        .unwrap();
        let unrelated = CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("unrelated"),
            WS_OVERLAPPED,
            0,
            0,
            100,
            100,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let window = WindowIdentity::from_handle(main.0 as isize).unwrap();
        assert!(window.owns_surface(main).unwrap());
        assert!(window.owns_surface(popup).unwrap());
        assert!(window.owns_surface(nested).unwrap());
        assert!(!window.owns_surface(unrelated).unwrap());
        assert!(!window.owns_surface(HWND::default()).unwrap());
        for handle in [nested, popup, unrelated, main] {
            DestroyWindow(handle).unwrap();
        }
        assert!(window.validate().is_err());
    }
}
