//! 只观察夹具主动报告的进程；不会按名字搜索或终止用户应用。
use std::{
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, Command},
};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

pub struct ObservedProcess(HANDLE);
impl ObservedProcess {
    pub fn open(pid: u32) -> Self {
        // SAFETY: PID 由本测试的子进程经专用本地端口提供，只请求同步权限。
        Self(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }.unwrap())
    }
    pub fn is_running(&self) -> bool {
        // SAFETY: 句柄存活并拥有同步权限，不等待或更改进程。
        let state = unsafe { WaitForSingleObject(self.0, 0) };
        assert!(state == WAIT_TIMEOUT || state == WAIT_OBJECT_0);
        state == WAIT_TIMEOUT
    }
}
impl Drop for ObservedProcess {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub struct ExternalFixture(Child);
impl ExternalFixture {
    pub fn start(path: &Path) -> Self {
        Self(
            Command::new(path)
                .creation_flags(0x0800_0000)
                .spawn()
                .unwrap(),
        )
    }
    pub fn is_running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }
}
impl Drop for ExternalFixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
