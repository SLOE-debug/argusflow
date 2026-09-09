//! 自有真实 Win32 控件，事件计数由窗口过程观察，不使用 UIA 替身。
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::HBRUSH,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::{HSTRING, w},
};

pub static LEFT: AtomicUsize = AtomicUsize::new(0);
pub static RIGHT: AtomicUsize = AtomicUsize::new(0);
pub static DOUBLE: AtomicUsize = AtomicUsize::new(0);
pub static VERTICAL: AtomicUsize = AtomicUsize::new(0);
pub static HORIZONTAL: AtomicUsize = AtomicUsize::new(0);

pub struct DpiContext(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
impl DpiContext {
    pub fn physical() -> Self {
        use windows::Win32::UI::HiDpi::*;
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        assert!(!previous.0.is_null());
        Self(previous)
    }
}
impl Drop for DpiContext {
    fn drop(&mut self) {
        unsafe { windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0) };
    }
}

pub struct Fixture {
    pub window: isize,
    thread: Option<JoinHandle<()>>,
}
impl Fixture {
    pub fn create() -> Self {
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            let instance = GetModuleHandleW(None).unwrap();
            let init = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_TREEVIEW_CLASSES,
            };
            InitCommonControlsEx(&init).unwrap();
            let class = w!("ArgusFlowNativeAcceptance");
            let definition = WNDCLASSW {
                style: CS_DBLCLKS,
                hInstance: instance.into(),
                lpszClassName: class,
                lpfnWndProc: Some(procedure),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
                hbrBackground: HBRUSH(6usize as *mut _),
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&definition), 0);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!("ArgusFlow Native Acceptance"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                760,
                650,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .unwrap();
            for (class, text, id, x, y, width, height, extra) in [
                (w!("BUTTON"), w!("Invoke target"), 101, 20, 20, 180, 40, 0),
                (w!("EDIT"), w!("initial"), 102, 20, 80, 300, 40, 0x00800000),
                (w!("BUTTON"), w!("Toggle target"), 103, 20, 140, 250, 35, 3),
                (w!("STATIC"), w!("Waiting"), 104, 20, 190, 300, 35, 0),
                (
                    w!("LISTBOX"),
                    w!("Choices"),
                    105,
                    370,
                    20,
                    300,
                    200,
                    0x00800001 | WS_VSCROLL.0,
                ),
            ] {
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
                )
                .unwrap();
                if id == 105 {
                    for index in 0..40 {
                        let text = HSTRING::from(format!("Item {index:02}"));
                        SendMessageW(
                            control,
                            LB_ADDSTRING,
                            Some(WPARAM(0)),
                            Some(LPARAM(text.as_ptr() as isize)),
                        );
                    }
                }
            }
            let tree = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("SysTreeView32"),
                w!("Tree"),
                WS_CHILD
                    | WS_VISIBLE
                    | WINDOW_STYLE(TVS_HASBUTTONS | TVS_HASLINES | TVS_LINESATROOT),
                20,
                250,
                300,
                150,
                Some(window),
                Some(HMENU(106usize as *mut _)),
                Some(instance.into()),
                None,
            )
            .unwrap();
            let parent = insert_tree(tree, HTREEITEM::default(), "Parent");
            insert_tree(tree, parent, "Child");
            let _ = SetForegroundWindow(window);
            sender.send(window.0 as isize).unwrap();
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        });
        Self {
            window: receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap(),
            thread: Some(thread),
        }
    }
    pub fn hwnd(&self) -> HWND {
        HWND(self.window as *mut _)
    }
    pub fn child(&self, id: i32) -> HWND {
        unsafe { GetDlgItem(Some(self.hwnd()), id) }.unwrap()
    }
    pub fn text(&self, id: i32) -> String {
        let mut buffer = [0u16; 256];
        let length = unsafe { GetWindowTextW(self.child(id), &mut buffer) } as usize;
        String::from_utf16_lossy(&buffer[..length])
    }
    pub fn message(&self, id: i32, message: u32) -> isize {
        unsafe { SendMessageW(self.child(id), message, None, None) }.0
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe {
            let _ = PostMessageW(Some(self.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: 唯一测试窗口过程，未调用任何外部应用窗口。
    unsafe {
        match message {
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            WM_COMMAND if wparam.0 & 0xffff == 101 => {
                let _ = SetWindowTextW(
                    GetDlgItem(Some(window), 104).unwrap(),
                    w!("Invoke received"),
                );
                return LRESULT(0);
            }
            WM_LBUTTONDOWN => {
                LEFT.fetch_add(1, Ordering::SeqCst);
            }
            WM_RBUTTONDOWN => {
                RIGHT.fetch_add(1, Ordering::SeqCst);
                return LRESULT(0);
            }
            WM_LBUTTONDBLCLK => {
                DOUBLE.fetch_add(1, Ordering::SeqCst);
            }
            WM_MOUSEWHEEL => {
                VERTICAL.fetch_add(1, Ordering::SeqCst);
                return LRESULT(0);
            }
            WM_MOUSEHWHEEL => {
                HORIZONTAL.fetch_add(1, Ordering::SeqCst);
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}
unsafe fn insert_tree(tree: HWND, parent: HTREEITEM, text: &str) -> HTREEITEM {
    let mut text = text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let item = TVITEMW {
        mask: TVIF_TEXT,
        pszText: windows::core::PWSTR(text.as_mut_ptr()),
        ..Default::default()
    };
    let insertion = TVINSERTSTRUCTW {
        hParent: parent,
        hInsertAfter: TVI_LAST,
        Anonymous: TVINSERTSTRUCTW_0 { item },
    };
    HTREEITEM(
        unsafe {
            SendMessageW(
                tree,
                TVM_INSERTITEMW,
                None,
                Some(LPARAM(&insertion as *const _ as isize)),
            )
        }
        .0,
    )
}
