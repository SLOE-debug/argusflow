//! 从操作系统与 UIA runtime health 读取 Planner 执行上下文。

use std::sync::Arc;

use argusflow_agent::{
    AccessibilityContext, ExecutionContext, ExecutionContextProvider, WindowContext,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::uia::UiaRuntimeHealth;

/// 每次 Planner prepare 前读取当前 Windows 前台窗口的上下文提供器。
#[derive(Debug)]
pub struct WindowsExecutionContextProvider {
    /// 与 UiaBackend 共享的唯一 runtime health。
    uia_health: Arc<UiaRuntimeHealth>,
}

impl WindowsExecutionContextProvider {
    /// 创建绑定真实 UIA runtime health 的上下文提供器。
    pub fn new(uia_health: Arc<UiaRuntimeHealth>) -> Self {
        Self { uia_health }
    }
}

impl ExecutionContextProvider for WindowsExecutionContextProvider {
    fn snapshot(&self) -> ExecutionContext {
        ExecutionContext {
            foreground_window: foreground_window_context(),
            accessibility: AccessibilityContext {
                ready: self.uia_health.is_ready(),
            },
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use argusflow_agent::ExecutionContextProvider;

    use super::WindowsExecutionContextProvider;
    use crate::uia::UiaRuntime;

    #[test]
    fn accessibility_context_reflects_shared_runtime_health() {
        let runtime = Arc::new(UiaRuntime::start());
        let provider = WindowsExecutionContextProvider::new(runtime.health());

        assert_eq!(
            provider.snapshot().accessibility.ready,
            runtime.health().is_ready()
        );
    }
}
