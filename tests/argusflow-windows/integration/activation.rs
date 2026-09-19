#![cfg(windows)]
//! 仅操作自有测试窗口，验证恢复、取消及置顶状态保持。
#[path = "../support/window.rs"]
#[allow(dead_code)]
mod support;
use argusflow_windows::{InputAction, InputService, OperationOptions, WindowIdentity};
use windows::Win32::UI::WindowsAndMessaging::*;

#[tokio::test]
#[ignore = "短暂使用真实桌面的自有窗口焦点回归"]
async fn activation_restores_minimized_window_and_preserves_topmost_state() {
    let fixture = support::Fixture::create();
    let window = WindowIdentity::from_handle(fixture.window).unwrap();
    let input = InputService::new().unwrap();
    for topmost in [false, true] {
        unsafe {
            SetWindowPos(
                fixture.hwnd(),
                Some(if topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                }),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .unwrap();
            let _ = ShowWindow(fixture.hwnd(), SW_MINIMIZE);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(unsafe { IsIconic(fixture.hwnd()) }.as_bool());
        let result = input
            .perform(
                window.clone(),
                InputAction::ActivateWindow,
                OperationOptions::default(),
            )
            .await;
        if result.is_err() {
            input.shutdown(OperationOptions::default()).await.unwrap();
        }
        result.unwrap();
        window.require_foreground().unwrap();
        assert!(!unsafe { IsIconic(fixture.hwnd()) }.as_bool());
        assert_eq!(
            unsafe { GetWindowLongPtrW(fixture.hwnd(), GWL_EXSTYLE) } & WS_EX_TOPMOST.0 as isize
                != 0,
            topmost
        );
        assert_eq!(fixture.text(102), "initial", "激活不得更改编辑区正文");
    }
    // 取消必须在任何恢复/移动效果之前生效。
    unsafe {
        let _ = ShowWindow(fixture.hwnd(), SW_HIDE);
    }
    let operation = argusflow_core::Operation::new(OperationOptions::default());
    operation.cancel();
    assert!(window.activate(&operation).is_err());
    assert!(!unsafe { IsWindowVisible(fixture.hwnd()) }.as_bool());
    input
        .perform(
            window.clone(),
            InputAction::ActivateWindow,
            OperationOptions::default(),
        )
        .await
        .unwrap();
    assert!(unsafe { IsWindowVisible(fixture.hwnd()) }.as_bool());
    window.require_foreground().unwrap();
    input.shutdown(OperationOptions::default()).await.unwrap();
}
