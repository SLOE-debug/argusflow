//! 可被输入和退出唤醒的高精度等待，不修改系统全局计时精度。
use std::sync::Arc;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Threading::*,
    },
    core::PCWSTR,
};

/// 内核事件与高精度定时器由同一生命周期管理。
pub(crate) struct SamplingWait {
    event: HANDLE,
    timer: HANDLE,
}

// SAFETY: 内核同步句柄允许跨线程等待/设置；只由最后一个 Arc 关闭。
unsafe impl Send for SamplingWait {}
unsafe impl Sync for SamplingWait {}

impl SamplingWait {
    pub(crate) fn new() -> Result<Arc<Self>, crate::RecorderError> {
        // SAFETY: 无名称、无自定义安全描述符，句柄由本类型接管。
        let event = unsafe { CreateEventW(None, false, false, None) }
            .map_err(|_| crate::RecorderError::WorkerUnavailable)?;
        let timer = match unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        } {
            Ok(timer) => timer,
            Err(_) => {
                unsafe {
                    let _ = CloseHandle(event);
                }
                return Err(crate::RecorderError::WorkerUnavailable);
            }
        };
        Ok(Arc::new(Self { event, timer }))
    }
    /// Hook 只设置事件，不访问像素或执行 GPU 命令。
    pub(crate) fn wake(&self) {
        unsafe {
            let _ = SetEvent(self.event);
        }
    }
    /// 0.5ms 检查预算为 GPU 提交与完成检查分别预留调度余量。
    pub(crate) fn wait(&self) -> Result<(), crate::RecorderError> {
        let due = -5_000i64;
        unsafe { SetWaitableTimerEx(self.timer, &due, 0, None, None, None, 0) }
            .map_err(|_| crate::RecorderError::WorkerUnavailable)?;
        let status = unsafe { WaitForMultipleObjects(&[self.event, self.timer], false, 1000) };
        if status == windows::Win32::Foundation::WAIT_FAILED {
            return Err(crate::RecorderError::WorkerUnavailable);
        }
        Ok(())
    }
}
impl Drop for SamplingWait {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.timer);
            let _ = CloseHandle(self.event);
        }
    }
}

impl argusflow_capture::CaptureScheduler for SamplingWait {
    fn wait(&self) -> Result<(), argusflow_core::CaptureFailure> {
        SamplingWait::wait(self).map_err(|_| argusflow_core::CaptureFailure::Unavailable)
    }
    fn wake(&self) {
        SamplingWait::wake(self);
    }
}
