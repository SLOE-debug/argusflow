//! 自有独立进程窗口；前台覆盖窗口可显式启用前台锁，用于可重复对比。
use super::Result;
use windows::{
    Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
    core::{HSTRING, w},
};
pub const ARM: u32 = WM_APP + 42;
pub const RELEASE: u32 = WM_APP + 43;
pub const REVEAL: u32 = WM_APP + 44;
pub const NORMAL_Z: u32 = WM_APP + 45;

pub fn run(kind: &str, ready: &str) -> Result<()> {
    let (style, extra) = match kind {
        "normal" | "cover" => (WS_OVERLAPPEDWINDOW, WINDOW_EX_STYLE::default()),
        "tool" => (WS_OVERLAPPEDWINDOW, WS_EX_TOOLWINDOW),
        "frameless" => (WS_POPUP, WS_EX_TOOLWINDOW),
        _ => return Err("未知测试窗口".into()),
    };
    // SAFETY: 本进程只创建一扇自有窗口，输出的 HWND 仅在本进程存活时可用。
    unsafe {
        let module = GetModuleHandleW(None)?;
        let class = w!("ArgusFlowActivationComparison");
        let definition = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        if RegisterClassW(&definition) == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        let title = HSTRING::from(format!("ArgusFlow Activation · {kind}"));
        let hwnd = CreateWindowExW(
            extra,
            class,
            &title,
            style | WS_VISIBLE,
            200,
            180,
            820,
            520,
            None,
            None,
            Some(module.into()),
            None,
        )?;
        // 普通文字子控件只用于直观看到被激活的对象，不含任何可执行业务按钮。
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            &HSTRING::from(format!("窗口激活对比 / {kind}\n本窗口不接收业务输入")),
            WS_CHILD | WS_VISIBLE,
            40,
            60,
            680,
            160,
            Some(hwnd),
            None,
            Some(module.into()),
            None,
        )?;
        std::fs::write(
            ready,
            serde_json::to_vec(
                &serde_json::json!({"hwnd":hwnd.0 as isize,"pid":std::process::id(),"kind":kind}),
            )?,
        )?;
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0).0;
            if status == -1 {
                return Err(windows::core::Error::from_thread().into());
            }
            if status == 0 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: 仅处理上述自建窗口的生命周期与诊断控制消息。
    unsafe {
        match message {
            ARM => return LRESULT(LockSetForegroundWindow(LSFW_LOCK).is_ok() as isize),
            RELEASE => {
                let _ = LockSetForegroundWindow(LSFW_UNLOCK);
                return LRESULT(1);
            }
            REVEAL => {
                return LRESULT(
                    SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                    )
                    .is_ok() as isize,
                );
            }
            NORMAL_Z => {
                return LRESULT(
                    SetWindowPos(
                        hwnd,
                        Some(HWND_NOTOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                    .is_ok() as isize,
                );
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}
pub fn control(hwnd: isize, message: u32) -> Result<bool> {
    let mut value = 0;
    // SAFETY: 测试协议仅发送至本 demo 创建的窗口，超时不无限等待。
    let result = unsafe {
        SendMessageTimeoutW(
            HWND(hwnd as *mut _),
            message,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            500,
            Some(&mut value),
        )
    };
    if result.0 == 0 {
        return Err("测试窗口控制消息超时".into());
    }
    Ok(value != 0)
}
