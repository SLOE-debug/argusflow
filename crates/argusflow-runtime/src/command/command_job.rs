//! Command 节点专用 Windows Job Object 进程树边界。

use std::{
    ffi::c_void,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};

use tokio::process::Child;
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject,
            },
            Threading::{CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    },
    core::PCWSTR,
};

/// 独占一次 Command 执行进程树的 Job Object。
pub(crate) struct CommandJob {
    /// 配置了 `KILL_ON_JOB_CLOSE` 的唯一 job handle。
    handle: OwnedHandle,
}

impl CommandJob {
    /// 要求根进程在任何用户代码运行前保持挂起，供 Job Object 原子接管生命周期。
    pub(crate) fn configure_command(command: &mut tokio::process::Command) {
        command.creation_flags(CREATE_SUSPENDED.0);
    }

    /// 创建一个在 handle 关闭时终止全部关联进程的无名 Job Object。
    pub(crate) fn create() -> Result<Self, String> {
        // SAFETY: 不传 security attributes 和名称，返回的唯一 handle 由 Drop 关闭。
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|error| format!("failed to create command job object: {error}"))?;
        // SAFETY: CreateJobObjectW 返回新的拥有型 handle，立即转交标准库 RAII 包装。
        let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // JOBOBJECT_EXTENDED_LIMIT_INFORMATION 的编译期结构大小远小于 u32 上限。
        let information_size = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        // SAFETY: limits 指向正确 information class 的完整只读结构，handle 在调用期间有效。
        unsafe {
            SetInformationJobObject(
                native_handle(&handle),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast::<c_void>(),
                information_size,
            )
        }
        .map_err(|error| format!("failed to configure command job object: {error}"))?;
        Ok(Self { handle })
    }

    /// 将挂起的根进程纳入 job，再恢复其唯一初始线程。
    pub(crate) fn assign_and_resume(&self, child: &Child) -> Result<(), String> {
        let process_handle = child
            .raw_handle()
            .map(HANDLE)
            .ok_or_else(|| "command process exited before job assignment".to_owned())?;
        let process_id = child
            .id()
            .ok_or_else(|| "command process id is unavailable before job assignment".to_owned())?;
        let primary_thread = find_process_thread(process_id)?;
        // SAFETY: 根进程以 CREATE_SUSPENDED 创建，尚不能派生逃逸 job 的后代；raw handle
        // 由仍存活的 tokio Child 借出，job 和 process 在调用期间均有效。
        unsafe { AssignProcessToJobObject(native_handle(&self.handle), process_handle) }
            .map_err(|error| format!("failed to assign command process to job object: {error}"))?;
        // SAFETY: thread handle 属于挂起根进程的初始线程，并包含 THREAD_SUSPEND_RESUME 权限。
        if unsafe { ResumeThread(native_handle(&primary_thread)) } == u32::MAX {
            return Err(format!(
                "failed to resume command process after job assignment: {}",
                windows::core::Error::from_thread()
            ));
        }
        Ok(())
    }

    /// 主动终止 job 中仍存活的根进程或后代进程。
    pub(crate) fn terminate(&self) {
        // SAFETY: handle 在 CommandJob 生命周期内有效；终止失败仍会由 Drop 的
        // KILL_ON_JOB_CLOSE 再次形成关闭边界。
        let _ = unsafe { TerminateJobObject(native_handle(&self.handle), 1) };
    }
}

/// 从系统线程快照中取得挂起根进程的唯一初始线程句柄。
fn find_process_thread(process_id: u32) -> Result<OwnedHandle, String> {
    // SAFETY: 全局线程快照不使用第二个参数，返回 handle 由 OwnedHandle 关闭。
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
        .map(|handle| {
            // SAFETY: snapshot API 返回新的拥有型 handle，立即转交标准库 RAII 包装。
            unsafe { OwnedHandle::from_raw_handle(handle.0) }
        })
        .map_err(|error| format!("failed to snapshot command process threads: {error}"))?;
    let mut entry = THREADENTRY32 {
        // THREADENTRY32 的编译期结构大小远小于 u32 上限。
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    // SAFETY: entry 是完整可写结构，snapshot 在枚举期间保持有效。
    unsafe { Thread32First(native_handle(&snapshot), &mut entry) }
        .map_err(|error| format!("failed to enumerate command process threads: {error}"))?;
    loop {
        if entry.th32OwnerProcessID == process_id {
            // SAFETY: thread id 来自有效系统快照，只请求恢复挂起线程所需的最小权限。
            return unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                .map(|handle| {
                    // SAFETY: OpenThread 返回新的拥有型 handle，立即转交标准库 RAII 包装。
                    unsafe { OwnedHandle::from_raw_handle(handle.0) }
                })
                .map_err(|error| format!("failed to open suspended command thread: {error}"));
        }
        // SAFETY: entry 和 snapshot 与 Thread32First 使用相同的有效缓冲区和句柄。
        if unsafe { Thread32Next(native_handle(&snapshot), &mut entry) }.is_err() {
            return Err("suspended command process thread was not found".to_owned());
        }
    }
}

/// 将标准库拥有型 handle 短暂借给 windows-rs API，不转移关闭责任。
fn native_handle(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}
