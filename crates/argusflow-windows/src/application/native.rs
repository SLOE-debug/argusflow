//! 挂起创建、加入自有 Job 后再恢复；失败路径不会运行未受控的子进程。
use super::{ApplicationOptions, command_line::command_line};
use super::{
    handle::{Handle, native},
    job::Job,
};
use crate::WindowsError;
use argusflow_core::Operation;
use std::os::windows::ffi::OsStrExt;
use windows::{
    Win32::{
        System::Threading::*,
        UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL},
    },
    core::{PCWSTR, PWSTR},
};

pub(super) struct NativeProcess {
    job: Job,
    process: Handle,
    pub pid: u32,
}
impl NativeProcess {
    pub fn launch(
        options: &ApplicationOptions,
        operation: &Operation,
    ) -> Result<Self, WindowsError> {
        let mut command = command_line(options.executable.as_os_str(), &options.arguments)?;
        let executable = options
            .executable
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let job = Job::new()?;
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            dwFlags: STARTF_USESHOWWINDOW,
            wShowWindow: if options.visible {
                SW_SHOWNORMAL.0 as u16
            } else {
                SW_HIDE.0 as u16
            },
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        let flags = CREATE_SUSPENDED
            | CREATE_UNICODE_ENVIRONMENT
            | if options.visible {
                PROCESS_CREATION_FLAGS(0)
            } else {
                CREATE_NO_WINDOW
            };
        operation.begin_effect("application_launch")?;
        // SAFETY: 字符串以 NUL 终止，命令行可写，句柄不继承，输出结构独占且有效。
        unsafe {
            CreateProcessW(
                PCWSTR(executable.as_ptr()),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                None,
                None,
                &startup,
                &mut info,
            )
        }
        .map_err(native)?;
        let thread = Handle(info.hThread);
        let process = Self {
            job,
            process: Handle(info.hProcess),
            pid: info.dwProcessId,
        };
        // process 的析构在任何后续失败路径终止刚创建的挂起进程。
        process.job.assign(process.process.0)?;
        operation.check("application_resume")?;
        // SAFETY: 线程来自本次 CREATE_SUSPENDED，未对外发布。
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            return Err(native(windows::core::Error::from_thread()));
        }
        Ok(process)
    }
    pub fn terminate(&self) -> Result<(), WindowsError> {
        self.job.terminate()
    }
    pub fn active_processes(&self) -> Result<u32, WindowsError> {
        self.job.active_processes()
    }
    pub fn exited(&self) -> Result<bool, WindowsError> {
        Ok(self.process.signaled()? && self.job.exited()?)
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        // SAFETY: 紧急 Drop 只终止自有进程；正常 shutdown 已确认进程树退出。
        unsafe {
            let _ = TerminateProcess(self.process.0, 1);
        }
        // job 的 KILL_ON_JOB_CLOSE 负责自有后代。
    }
}
