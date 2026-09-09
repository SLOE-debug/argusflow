//! 原生工作线程的 DPI 上下文守卫。
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
pub(super) struct DpiScope(DPI_AWARENESS_CONTEXT);
impl DpiScope {
    pub fn enter() -> Self {
        // SAFETY: 仅修改调用线程，守卫在同一线程恢复旧上下文。
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}
impl Drop for DpiScope {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}
