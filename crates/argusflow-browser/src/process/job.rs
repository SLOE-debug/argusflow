//! Windows Job 限定自建浏览器树的所有权，外部连接不会进入 Job。
use crate::BrowserError as Failure;
use argusflow_core::FailureKind;
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
};

pub(crate) struct Job(HANDLE);
// SAFETY: Job 是内核句柄，支持跨线程关闭；不包含 COM 或线程关联状态。
unsafe impl Send for Job {}
impl Job {
    pub(crate) fn assign(child: &tokio::process::Child) -> Result<Self, Failure> {
        // SAFETY: 无名 Job，结构体长度与 Windows ABI 匹配。
        let job = Self(unsafe { CreateJobObjectW(None, None) }.map_err(native)?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(native)?;
        let handle = child.raw_handle().ok_or_else(|| {
            Failure::new(FailureKind::Unavailable, "browser_job", "浏览器进程已退出")
        })?;
        // SAFETY: Child 在调用期间保持进程句柄有效。
        unsafe { AssignProcessToJobObject(job.0, HANDLE(handle)) }.map_err(native)?;
        Ok(job)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn native(error: windows::core::Error) -> Failure {
    Failure::new(
        FailureKind::Unavailable,
        "browser_job",
        "无法建立浏览器进程树所有权",
    )
    .with_source(error)
}
