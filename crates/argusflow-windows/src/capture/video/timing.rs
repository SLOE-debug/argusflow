//! QPC至媒体100ns时钟的整数映射。
use super::model::{Result, VideoError};
use crate::capture::clock::Clock;
use windows::Win32::System::Performance::QueryPerformanceCounter;
pub(super) fn qpc() -> Result<i64> {
    let mut value = 0;
    // SAFETY: 有效局部输出指针。
    unsafe {
        QueryPerformanceCounter(&mut value)?;
    }
    Ok(value)
}
pub(super) fn pts(qpc: i64, clock: Clock) -> i64 {
    (i128::from(qpc.saturating_sub(clock.0.origin).max(0)) * 10_000_000
        / i128::from(clock.0.frequency)) as i64
}

pub(super) fn duration(start: i64, end: i64) -> Result<i64> {
    end.checked_sub(start)
        .filter(|v| *v > 0)
        .ok_or_else(|| VideoError::Invalid("视频样本时间必须严格递增".into()))
}

/// 原生MP4视频时基可精确表达整毫秒。先量化绝对边界再求duration，避免逐帧舍入漂移。
/// 呈现QPC另行原样保存；视频边界量化误差最多0.5ms。
pub(super) fn media_pts(time_100ns: i64) -> i64 {
    time_100ns.saturating_add(5_000) / 10_000 * 10_000
}
