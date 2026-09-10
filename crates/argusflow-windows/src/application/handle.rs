//! 进程生命周期使用的内核句柄与无阻塞退出检查。
use crate::WindowsError;
use argusflow_core::FailureKind;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::WaitForSingleObject,
};

pub(super) struct Handle(pub HANDLE);
// SAFETY: 只持有内核进程、线程或 Job 句柄，Win32 允许跨线程查询与关闭。
unsafe impl Send for Handle {}
// SAFETY: 对象状态由内核同步，不访问用户内存。
unsafe impl Sync for Handle {}
impl Handle {
    pub fn signaled(&self) -> Result<bool, WindowsError> {
        // SAFETY: 本对象保持进程句柄有效；零超时查询不会阻塞执行线程。
        match unsafe { WaitForSingleObject(self.0, 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(native(windows::core::Error::from_thread())),
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub(super) fn native(error: windows::core::Error) -> WindowsError {
    WindowsError::new(
        FailureKind::Native,
        "application_process",
        "应用进程生命周期操作失败",
    )
    .with_source(error)
}
