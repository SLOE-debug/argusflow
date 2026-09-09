//! Win32 错误和 apartment 生命周期的最小 unsafe 边界。
use crate::WindowsError as Failure;
use argusflow_core::FailureKind;
use windows::Win32::{
    Foundation::HWND,
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
};

pub(crate) fn failure(stage: &'static str, error: windows::core::Error) -> Failure {
    let kind = match error.code().0 as u32 {
        0x80040201 => FailureKind::StaleHandle,
        0x80040204 => FailureKind::Unsupported,
        0x80131505 => FailureKind::Timeout,
        _ => FailureKind::Native,
    };
    Failure::new(
        kind,
        stage,
        format!("Win32 HRESULT 0x{:08x}", error.code().0 as u32),
    )
    .with_source(error)
}

pub(crate) fn pattern_failure(stage: &'static str, error: windows::core::Error) -> Failure {
    if error.code().is_ok() || error.code().0 as u32 == 0x80004002 {
        Failure::new(
            FailureKind::Unsupported,
            stage,
            "控件未提供请求的 UIA Pattern",
        )
        .with_source(error)
    } else {
        failure(stage, error)
    }
}

pub(crate) fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut std::ffi::c_void)
}

/// 只能在创建线程销毁，防止 apartment 生命周期跨线程。
pub(crate) struct Apartment {
    _thread_bound: std::marker::PhantomData<*mut ()>,
}
impl Apartment {
    pub(crate) fn new() -> Result<Self, Failure> {
        // SAFETY: 当前专用线程不拥有窗口，在退出前恰好调用一次 CoUninitialize。
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| failure("com_initialize", error))?;
        Ok(Self {
            _thread_bound: std::marker::PhantomData,
        })
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: 守卫无法 Send，且只在成功初始化后构造。
        unsafe { CoUninitialize() };
    }
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/platform/com.rs"]
mod tests;
