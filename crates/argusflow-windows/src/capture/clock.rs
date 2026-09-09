//! 原生 QPC 与 WGC 100ns 时间统一为单调微秒。

use argusflow_core::InspectionFailure;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

pub(super) fn qpc_us(value: i64) -> Result<u64, InspectionFailure> {
    let mut frequency = 0;
    // SAFETY: 两个 API 只写入有效的栈上整数槽。
    unsafe { QueryPerformanceFrequency(&mut frequency) }
        .map_err(|_| InspectionFailure::Unavailable)?;
    if value < 0 || frequency <= 0 {
        return Err(InspectionFailure::Unavailable);
    }
    Ok((value as u128 * 1_000_000 / frequency as u128) as u64)
}

pub(super) fn now_us() -> Result<u64, InspectionFailure> {
    let mut value = 0;
    unsafe { QueryPerformanceCounter(&mut value) }.map_err(|_| InspectionFailure::Unavailable)?;
    qpc_us(value)
}
