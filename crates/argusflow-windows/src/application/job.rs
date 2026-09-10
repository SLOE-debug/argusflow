//! 自有 Job 的有界进程树；清理需同时确认计数归零和进程句柄退出。
use super::handle::{Handle, native};
use crate::WindowsError;
use argusflow_core::FailureKind;
use std::{collections::BTreeMap, sync::Mutex};
use windows::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, HANDLE},
    System::{
        JobObjects::*,
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE},
    },
};
use windows::core::BOOL;

const PROCESS_LIMIT: usize = 128;
#[repr(C)]
struct ProcessIds {
    assigned: u32,
    count: u32,
    ids: [usize; PROCESS_LIMIT],
}
pub(super) struct Job {
    handle: Handle,
    waiting: Mutex<BTreeMap<usize, Handle>>,
}
impl Job {
    pub fn new() -> Result<Self, WindowsError> {
        // SAFETY: 未命名 Job 由本对象独占所有权。
        let job = Self {
            handle: Handle(unsafe { CreateJobObjectW(None, None) }.map_err(native)?),
            waiting: Mutex::new(BTreeMap::new()),
        };
        job.limit(PROCESS_LIMIT as u32)?;
        Ok(job)
    }
    fn limit(&self, active: u32) -> Result<(), WindowsError> {
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = active;
        // SAFETY: 结构及长度与信息类别一致，调用不保留指针。
        unsafe {
            SetInformationJobObject(
                self.handle.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(native)
    }
    pub fn assign(&self, process: HANDLE) -> Result<(), WindowsError> {
        // SAFETY: 调用者保持本次创建的挂起进程句柄有效。
        unsafe { AssignProcessToJobObject(self.handle.0, process) }.map_err(native)
    }
    pub fn terminate(&self) -> Result<(), WindowsError> {
        // 关闭阶段先阻止成员继续成功创建后代，再取得稳定的有界 PID 集合。
        self.limit(1)?;
        let mut ids = ProcessIds {
            assigned: 0,
            count: 0,
            ids: [0; PROCESS_LIMIT],
        };
        // SAFETY: repr(C) 首部及尾随 ULONG_PTR 数组与 Win32 可变长结构一致。
        unsafe {
            QueryInformationJobObject(
                Some(self.handle.0),
                JobObjectBasicProcessIdList,
                &mut ids as *mut _ as _,
                std::mem::size_of_val(&ids) as u32,
                None,
            )
        }
        .map_err(native)?;
        if ids.count as usize > PROCESS_LIMIT || ids.count < ids.assigned {
            return Err(WindowsError::new(
                FailureKind::ResourceLimit,
                "application_shutdown",
                "自有进程列表超过有界缓冲区",
            ));
        }
        let mut handles = Vec::new();
        for pid in &ids.ids[..ids.count as usize] {
            // SAFETY: 只查询 Job 提供的 PID，不申请终止权限；持有句柄后再次检查归属。
            let process = match unsafe {
                OpenProcess(
                    PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    *pid as u32,
                )
            } {
                Ok(handle) => Handle(handle),
                Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => continue,
                Err(error) => return Err(native(error)),
            };
            let mut belongs = BOOL::default();
            unsafe { IsProcessInJob(process.0, Some(self.handle.0), &mut belongs) }
                .map_err(native)?;
            if belongs.as_bool() {
                handles.push((*pid, process));
            }
        }
        self.waiting.lock().map_err(|_| poisoned())?.extend(handles);
        // SAFETY: 只终止本对象创建的 Job 成员，不按 PID 或进程名终止外部对象。
        unsafe { TerminateJobObject(self.handle.0, 1) }.map_err(native)
    }
    pub fn active_processes(&self) -> Result<u32, WindowsError> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(self.handle.0),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .map_err(native)?;
        Ok(info.ActiveProcesses)
    }
    pub fn exited(&self) -> Result<bool, WindowsError> {
        if self.active_processes()? != 0 {
            return Ok(false);
        }
        let waiting = self.waiting.lock().map_err(|_| poisoned())?;
        for process in waiting.values() {
            if !process.signaled()? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
fn poisoned() -> WindowsError {
    WindowsError::new(
        FailureKind::Native,
        "application_shutdown",
        "应用进程清理锁损坏",
    )
}
