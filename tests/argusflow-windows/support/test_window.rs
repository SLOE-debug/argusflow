//! 手动运行的独立 Win32 控件窗口；不自动启动或注入输入。
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows::{
        Win32::{
            Foundation::*, Graphics::Gdi::HBRUSH, System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::*,
        },
        core::w,
    };
    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: Win32 message loop 提供有效窗口参数，只处理本示例的控件。
        unsafe {
            if message == WM_DESTROY {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            if message == WM_COMMAND && wparam.0 & 0xffff == 101 {
                if let Ok(label) = GetDlgItem(Some(window), 104) {
                    let _ = SetWindowTextW(label, w!("Invoke received"));
                }
                return LRESULT(0);
            }
            DefWindowProcW(window, message, wparam, lparam)
        }
    }
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("ArgusFlowTestWindow");
        let definition = WNDCLASSW {
            hInstance: instance.into(),
            lpszClassName: class,
            lpfnWndProc: Some(procedure),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(6usize as *mut _),
            ..Default::default()
        };
        if RegisterClassW(&definition) == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("ArgusFlow UIA Test"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            680,
            400,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let controls = [
            (w!("BUTTON"), w!("Invoke me"), 101, 20, 20, 200, 40, 0),
            (
                w!("EDIT"),
                w!("Editable value"),
                102,
                20,
                80,
                400,
                40,
                0x00800000,
            ),
            (w!("BUTTON"), w!("Toggle me"), 103, 20, 140, 220, 35, 3),
            (w!("STATIC"), w!("Waiting"), 104, 20, 190, 400, 35, 0),
            (
                w!("LISTBOX"),
                w!("Choices"),
                105,
                450,
                20,
                180,
                180,
                0x00800001,
            ),
        ];
        for (class, text, id, x, y, width, height, extra) in controls {
            let control = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                text,
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(extra),
                x,
                y,
                width,
                height,
                Some(window),
                Some(HMENU(id as *mut _)),
                Some(instance.into()),
                None,
            )?;
            if id == 105 {
                SendMessageW(
                    control,
                    LB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(w!("First").as_ptr() as isize)),
                );
                SendMessageW(
                    control,
                    LB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(w!("Second").as_ptr() as isize)),
                );
            }
        }
        println!("HWND={} PID={}", window.0 as isize, std::process::id());
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
#[cfg(not(windows))]
fn main() {
    eprintln!("This fixture requires Windows.");
}
