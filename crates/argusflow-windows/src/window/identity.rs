//! HWND、进程创建时间与窗口属性租约；不拥有目标应用。
use crate::WindowsError as Failure;
use crate::platform::{failure, hwnd};
use argusflow_core::FailureKind;
use windows::Win32::{
    Foundation::{CloseHandle, FILETIME},
    System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, IsWindow},
};

/// 带进程创建时间与窗口属性租约的身份，阻止 HWND/PID 复用误操作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowIdentity {
    handle: isize,
    process_id: u32,
    process_created: u64,
    stamp: std::sync::Arc<super::stamp::Stamp>,
}

impl WindowIdentity {
    /// 验证 HWND、捕获进程身份并建立窗口属性租约；UIPI 权限不足明确失败。
    pub fn from_handle(handle: isize) -> Result<Self, Failure> {
        let mut process_id = 0;
        // SAFETY: 仅查询外部窗口，不解引用句柄。
        if !unsafe { IsWindow(Some(hwnd(handle))) }.as_bool() {
            return Err(stale());
        }
        // SAFETY: process_id 是同步调用期间有效的独占输出。
        unsafe { GetWindowThreadProcessId(hwnd(handle), Some(&mut process_id)) };
        Ok(Self {
            handle,
            process_id,
            process_created: creation_time(process_id)?,
            stamp: super::stamp::capture(hwnd(handle))?,
        })
    }
    /// 操作前重新读取 HWND/PID/创建时间。
    pub fn validate(&self) -> Result<(), Failure> {
        if !self.stamp.valid() {
            return Err(stale());
        }
        if Self::from_handle(self.handle)? != *self {
            return Err(stale());
        }
        Ok(())
    }
    /// HWND 的平台整数表示。
    pub fn handle(&self) -> isize {
        self.handle
    }
    /// 所属进程 ID。
    pub fn process_id(&self) -> u32 {
        self.process_id
    }
    /// 验证窗口当前在前台，不擅自抢焦点。
    pub fn require_foreground(&self) -> Result<(), Failure> {
        self.validate()?;
        // SAFETY: GetForegroundWindow 是无参数只读查询。
        if unsafe { GetForegroundWindow() } != hwnd(self.handle) {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "foreground",
                "目标窗口当前不在前台",
            ));
        }
        Ok(())
    }
}

fn creation_time(pid: u32) -> Result<u64, Failure> {
    // SAFETY: 只申请读取进程元数据的权限，句柄在所有成功路径关闭。
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|error| failure("open_process", error))?;
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: 输出互不重叠，process 在调用期间存活。
    let result =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    // SAFETY: 该函数唯一拥有此句柄。
    unsafe { CloseHandle(process) }.map_err(|error| failure("close_process", error))?;
    result.map_err(|error| failure("process_identity", error))?;
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

fn stale() -> Failure {
    Failure::new(
        FailureKind::StaleHandle,
        "window_identity",
        "目标窗口或进程已经失效",
    )
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/window/identity.rs"]
mod tests;
