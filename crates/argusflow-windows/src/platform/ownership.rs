//! 原生调用未退出前保留实例占用，禁止取消初始化后不断创建替代线程。
use crate::WindowsError as Failure;
use argusflow_core::FailureKind;
use std::sync::atomic::{AtomicBool, Ordering};
static UIA: AtomicBool = AtomicBool::new(false);
static INPUT: AtomicBool = AtomicBool::new(false);
pub(crate) struct Owner(&'static AtomicBool);
impl Owner {
    pub(crate) fn uia() -> Result<Self, Failure> {
        Self::acquire(&UIA, "uia_start")
    }
    pub(crate) fn input() -> Result<Self, Failure> {
        Self::acquire(&INPUT, "input_start")
    }
    fn acquire(slot: &'static AtomicBool, stage: &'static str) -> Result<Self, Failure> {
        slot.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Failure::new(
                    FailureKind::Busy,
                    stage,
                    "已有实例或尚未退出的原生线程；请复用现有句柄或等待 shutdown",
                )
            })?;
        Ok(Self(slot))
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
