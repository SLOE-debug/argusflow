//! 在原生线程中统一 UIA 几何与真实输入的物理像素坐标。
use std::{marker::PhantomData, rc::Rc};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};

pub(crate) struct PhysicalDpi {
    previous: DPI_AWARENESS_CONTEXT,
    _thread: PhantomData<Rc<()>>,
}

impl PhysicalDpi {
    pub(crate) fn enter() -> Result<Self, crate::WindowsError> {
        // SAFETY: 只调整调用线程；守卫不可跨线程，退出时恢复原上下文。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err(super::failure(
                "uia_dpi",
                windows::core::Error::from_thread(),
            ));
        }
        Ok(Self {
            previous,
            _thread: PhantomData,
        })
    }
}

impl Drop for PhysicalDpi {
    fn drop(&mut self) {
        // SAFETY: previous 来自本线程成功的上下文切换。
        unsafe {
            SetThreadDpiAwarenessContext(self.previous);
        }
    }
}
