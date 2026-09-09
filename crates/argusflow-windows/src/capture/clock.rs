//! 服务共享的 QPC 起点，保留呈现与观察之间的单调关系。
use argusflow_capture_contracts::{CaptureResult, ClockDomain, ClockTime};
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
static SESSION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy)]
pub(super) struct Clock(pub ClockDomain);
impl Clock {
    pub fn new() -> CaptureResult<Self> {
        let mut origin = 0;
        let mut frequency = 0;
        // SAFETY: 独占有效输出指针，Windows 10+ 提供稳定的 QPC。
        unsafe {
            QueryPerformanceCounter(&mut origin).map_err(super::gpu::failure)?;
            QueryPerformanceFrequency(&mut frequency).map_err(super::gpu::failure)?;
        }
        if frequency <= 0 {
            return Err(super::gpu::invalid("QPC frequency"));
        }
        let nonce = SESSION.fetch_add(1, Ordering::Relaxed);
        Ok(Self(ClockDomain {
            session: ((origin as u128) << 64)
                | (u128::from(std::process::id()) << 32)
                | u128::from(nonce),
            origin,
            frequency: frequency as u64,
        }))
    }
    pub fn convert(self, qpc: i64) -> ClockTime {
        ClockTime(
            ((qpc.saturating_sub(self.0.origin).max(0) as u128) * 1_000_000_000
                / u128::from(self.0.frequency))
            .min(u128::from(u64::MAX)) as u64,
        )
    }
    pub fn now(self) -> ClockTime {
        let mut value = self.0.origin;
        // SAFETY: QPC 在受支持 Windows 上不会失败；失败值保留起点，不伪造推进水位。
        let _ = unsafe { QueryPerformanceCounter(&mut value) };
        self.convert(value)
    }
}
