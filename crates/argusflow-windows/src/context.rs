//! 从操作系统读取、且不依赖 UI 状态的 Planner 执行上下文。

use argusflow_agent::{ExecutionContext, ExecutionContextProvider, WindowContext};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// 每次 Planner prepare 前读取当前 Windows 前台窗口的上下文提供器。
#[derive(Debug, Default)]
pub struct WindowsExecutionContextProvider;

impl ExecutionContextProvider for WindowsExecutionContextProvider {
    fn snapshot(&self) -> ExecutionContext {
        ExecutionContext {
            foreground_window: foreground_window_context(),
            ..ExecutionContext::default()
        }
    }
}

/// 读取当前前台 HWND 与所属进程 ID；无前台窗口时返回 `None`。
fn foreground_window_context() -> Option<WindowContext> {
    // SAFETY: 两个 User32 调用只读取当前线程外的窗口状态；HWND 仅转换为不透明数值，
    // 不在调用返回后解引用。`process_id` 指针在调用期间有效且独占。
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    let mut process_id = 0_u32;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut process_id));
    }
    (process_id != 0).then_some(WindowContext {
        handle: window.0 as usize as u64,
        process_id,
    })
}
