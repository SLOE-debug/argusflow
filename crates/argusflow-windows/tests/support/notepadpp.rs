//! 独立 Notepad++ 进程、PID scoped HWND 与失败清理。

use std::{
    env,
    ffi::c_void,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use argusflow_agent::WindowContext;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
        },
    },
    core::BOOL,
};

use super::uia_dump::dump_control_view;

/// 启动参数和 HWND 都与用户已有 Notepad++ session 隔离的夹具。
pub(crate) struct NotepadPlusPlus {
    /// 测试创建且负责清理的子进程。
    child: Child,
    /// 通过 PID 枚举得到的顶层窗口。
    window: WindowContext,
}

impl NotepadPlusPlus {
    /// 从 `ARGUSFLOW_NOTEPADPP_EXE` 启动固定英文测试环境要求的进程。
    pub(crate) fn launch() -> Self {
        let executable = env::var_os("ARGUSFLOW_NOTEPADPP_EXE").unwrap_or_else(|| {
            panic!("ARGUSFLOW_NOTEPADPP_EXE must point to a 64-bit English Notepad++ executable")
        });
        let mut child = Command::new(executable)
            .args(["-multiInst", "-nosession", "-noPlugin"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Notepad++ should start");
        let process_id = child.id();
        let timeout = Duration::from_secs(15);
        let Some(window) = wait_for_window(process_id, timeout) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Notepad++ did not expose a visible top-level HWND within {timeout:?}");
        };
        Self { child, window }
    }

    /// 返回可注入 StaticExecutionContext 的稳定 HWND/PID。
    pub(crate) fn window(&self) -> WindowContext {
        self.window.clone()
    }
}

impl Drop for NotepadPlusPlus {
    fn drop(&mut self) {
        if thread::panicking() {
            dump_control_view(&self.window, 6);
        }
        let hwnd = HWND(self.window.handle as usize as *mut c_void);
        // SAFETY: HWND 来自按子进程 PID 枚举的顶层窗口，仅用于投递 WM_CLOSE。
        let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 在指定期限内按 PID 枚举唯一可见顶层窗口。
fn wait_for_window(process_id: u32, timeout: Duration) -> Option<WindowContext> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(handle) = find_visible_window(process_id) {
            return Some(WindowContext { handle, process_id });
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

/// 同步 EnumWindows 回调使用的查找状态。
struct WindowSearch {
    /// 目标子进程 ID。
    process_id: u32,
    /// 首个匹配的可见顶层 HWND。
    handle: Option<u64>,
}

/// 返回属于目标 PID 的首个可见顶层窗口。
fn find_visible_window(process_id: u32) -> Option<u64> {
    let mut search = WindowSearch {
        process_id,
        handle: None,
    };
    let parameter = LPARAM((&mut search as *mut WindowSearch) as isize);
    // SAFETY: callback 与 parameter 在同步 EnumWindows 调用期间保持有效。
    let _ = unsafe { EnumWindows(Some(enum_window), parameter) };
    search.handle
}

/// `EnumWindows` 的同步回调；找到目标后返回 FALSE 提前停止枚举。
unsafe extern "system" fn enum_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: parameter 指向 find_visible_window 栈上状态，EnumWindows 在函数返回前同步调用。
    let search = unsafe { &mut *(parameter.0 as *mut WindowSearch) };
    let mut process_id = 0_u32;
    // SAFETY: process id 指针在同步回调期间有效且独占，window 由 EnumWindows 提供。
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut process_id));
    }
    // SAFETY: window 由当前 EnumWindows 回调提供，仅执行只读可见性查询。
    if process_id == search.process_id && unsafe { IsWindowVisible(window) }.as_bool() {
        search.handle = Some(window.0 as usize as u64);
        BOOL(0)
    } else {
        BOOL(1)
    }
}
