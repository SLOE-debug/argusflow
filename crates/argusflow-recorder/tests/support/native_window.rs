//! 有明确生命周期的 Win32 fixture；窗口、控件、focus 和消息泵始终属于同一线程。

use argusflow_core::ScreenPoint;
use std::{sync::mpsc, thread::JoinHandle};
use windows::{
    Win32::{
        Foundation::{LPARAM, POINT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext},
            Input::KeyboardAndMouse::{SetFocus, VK_RETURN},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

/// 只将不透明 HWND 数值、物理点及停止线程暴露给测试。
pub struct NativeWindow {
    /// 本 fixture 唯一根 HWND。
    pub handle: u64,
    /// 普通 Edit 内的屏幕物理命中点。
    pub ordinary: ScreenPoint,
    /// Password Edit 内的屏幕物理命中点。
    pub password: ScreenPoint,
    /// 只通知测试窗口内的 Enter 释放，不传递任何文字。
    completion: mpsc::Receiver<()>,
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl NativeWindow {
    /// 创建两个可由 UIA 反查的实际原生控件。
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (finished, completion) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            // SAFETY: fixture GUI 专用线程；所有窗口在同一个线程销毁。
            unsafe {
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                let module = GetModuleHandleW(None).unwrap();
                let root = CreateWindowExW(
                    WS_EX_TOPMOST,
                    w!("STATIC"),
                    w!("ArgusFlow recorder native test (auto closes)"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    120,
                    120,
                    560,
                    230,
                    None,
                    None,
                    Some(module.into()),
                    None,
                )
                .unwrap();
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Ordinary test field"),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    15,
                    500,
                    25,
                    Some(root),
                    None,
                    Some(module.into()),
                    None,
                )
                .unwrap();
                let ordinary = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                    20,
                    50,
                    490,
                    28,
                    Some(root),
                    Some(HMENU(1001usize as *mut _)),
                    Some(module.into()),
                    None,
                )
                .unwrap();
                let password = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    w!("EDIT"),
                    w!(""),
                    WS_CHILD
                        | WS_VISIBLE
                        | WS_TABSTOP
                        | WINDOW_STYLE((ES_PASSWORD | ES_AUTOHSCROLL) as u32),
                    20,
                    100,
                    490,
                    28,
                    Some(root),
                    Some(HMENU(1002usize as *mut _)),
                    Some(module.into()),
                    None,
                )
                .unwrap();
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Top: argus | Bottom: secret42 | Enter to finish"),
                    WS_CHILD | WS_VISIBLE,
                    20,
                    145,
                    510,
                    24,
                    Some(root),
                    None,
                    Some(module.into()),
                    None,
                )
                .unwrap();
                let mut point_a = POINT { x: 40, y: 65 };
                let mut point_b = POINT { x: 40, y: 115 };
                assert!(ClientToScreen(root, &mut point_a).as_bool());
                assert!(ClientToScreen(root, &mut point_b).as_bool());
                let mut message = MSG::default();
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                sender
                    .send((
                        root.0 as u64,
                        GetCurrentThreadId(),
                        point_a.x,
                        point_a.y,
                        point_b.x,
                        point_b.y,
                    ))
                    .unwrap();
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                    if message.message == WM_APP {
                        let _ = SetForegroundWindow(root);
                        let _ = SetFocus(Some(if message.wParam.0 == 1 {
                            password
                        } else {
                            ordinary
                        }));
                    } else {
                        if message.message == WM_KEYUP && message.wParam.0 == VK_RETURN.0 as usize {
                            let _ = finished.try_send(());
                        }
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                if IsWindow(Some(root)).as_bool() {
                    DestroyWindow(root).unwrap();
                }
            }
        });
        let (handle, thread_id, x1, y1, x2, y2) = receiver.recv().unwrap();
        Self {
            handle,
            thread_id,
            completion,
            ordinary: ScreenPoint { x: x1, y: y1 },
            password: ScreenPoint { x: x2, y: y2 },
            thread: Some(thread),
        }
    }

    /// 为焦点反查测试切换到密码框，不合成输入事件。
    pub fn focus_password(&self) {
        // SAFETY: 向自己的 fixture GUI 线程发送无指针 focus 请求。
        unsafe {
            PostThreadMessageW(self.thread_id, WM_APP, WPARAM(1), LPARAM(0)).unwrap();
        }
    }

    /// 用户已在测试窗口内释放 Enter；不会消费其他应用按键。
    pub fn finished(&self) -> bool {
        self.completion.try_recv().is_ok()
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        // SAFETY: 只停止本 fixture 创建的线程；Join 确保窗口已经销毁。
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
