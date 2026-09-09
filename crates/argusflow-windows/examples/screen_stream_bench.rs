//! 自有窗口固定 50ms 呈现探针，只输出计数与延迟，不保存桌面内容。
use argusflow_capture::{CaptureBroker, PixelRect};
use argusflow_core::capture::ScreenCaptureSource;
use argusflow_windows::capture::WindowsEventCapture;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
#[path = "support/capture_fixture.rs"]
mod fixture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let width: i32 = args.first().ok_or("width required")?.parse()?;
    let height: i32 = args.get(1).ok_or("height required")?.parse()?;
    let (ready, started) = mpsc::sync_channel(1);
    let (begin, gate) = mpsc::sync_channel(1);
    let (finish, finished) = mpsc::sync_channel(1);
    let painter = std::thread::spawn(move || -> Result<(), String> {
        let fixture =
            fixture::CaptureFixture::new(width, height).map_err(|error| error.to_string())?;
        fixture.paint(0).map_err(|error| error.to_string())?;
        ready.send(()).map_err(|error| error.to_string())?;
        gate.recv().map_err(|error| error.to_string())?;
        let origin = Instant::now();
        for step in 1..=108 {
            let deadline = origin + Duration::from_millis(step as u64 * 50);
            if let Some(delay) = deadline.checked_duration_since(Instant::now()) {
                std::thread::sleep(delay);
            }
            fixture
                .paint_patch(step)
                .map_err(|error| error.to_string())?;
        }
        let _ = finished.recv_timeout(Duration::from_secs(3));
        Ok(())
    });
    if started.recv_timeout(Duration::from_secs(3)).is_err() {
        return Err(painter
            .join()
            .map_err(|_| "painter panic")?
            .err()
            .unwrap_or("fixture startup timed out".into())
            .into());
    }
    let capture = WindowsEventCapture::default();
    let timer = ProbeTimer::new()?;
    let sources = capture.sources()?;
    let mut broker = CaptureBroker::default();
    let mut begun = false;
    let mut observed = std::collections::BTreeSet::new();
    let mut latencies = Vec::new();
    let mut readback_bytes = 0usize;
    let mut diff_us = 0u128;
    let mut accumulated = 0u64;
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline && observed.len() < 108 {
        for update in capture.poll()? {
            readback_bytes += update
                .patches
                .iter()
                .map(|patch| patch.pixels().len())
                .sum::<usize>();
            accumulated += u64::from(update.accumulated_frames.saturating_sub(1));
            let latency = update
                .timing
                .frozen_us
                .saturating_sub(update.timing.presented_us);
            let start = Instant::now();
            let published = broker
                .publish(update)
                .map_err(|error| format!("capture error: {error:?}"))?;
            diff_us += start.elapsed().as_micros();
            if let Some(frame) = published {
                let bounds = frame.snapshot.bounds();
                if bounds.x <= 8.0
                    && bounds.y <= 8.0
                    && bounds.x + bounds.width > 8.0
                    && bounds.y + bounds.height > 8.0
                {
                    let pixel = frame
                        .snapshot
                        .crop(PixelRect {
                            x: (8.0 - bounds.x) as u32,
                            y: (8.0 - bounds.y) as u32,
                            width: 1,
                            height: 1,
                        })
                        .map_err(|error| format!("crop: {error:?}"))?;
                    let p = pixel.pixels();
                    if p[0] == 20
                        && p[1] == 10
                        && (1..=108).contains(&p[2])
                        && observed.insert(p[2])
                    {
                        latencies.push(latency);
                    }
                }
            }
        }
        if !begun
            && sources
                .iter()
                .all(|source| broker.source(*source).is_some())
        {
            begin.send(())?;
            begun = true;
        }
        timer.wait()?;
    }
    let _ = finish.send(());
    painter.join().map_err(|_| "painter panic")??;
    latencies.sort_unstable();
    let p95 = latencies
        .get((latencies.len() * 95 / 100).min(latencies.len().saturating_sub(1)))
        .copied()
        .unwrap_or(0);
    println!(
        "fixture={}x{} interval_ms=50 first_eight={}/8 observed={}/108 missing={} accumulated_presents={} readback_bytes={} diff_total_us={} present_to_freeze_p95_us={}",
        width,
        height,
        (1..=8).filter(|step| observed.contains(step)).count(),
        observed.len(),
        108 - observed.len(),
        accumulated,
        readback_bytes,
        diff_us,
        p95
    );
    Ok(())
}

/// 与生产调度一样使用高精度 0.5ms 定时器，避免普通 Sleep 的时钟量化。
struct ProbeTimer(windows::Win32::Foundation::HANDLE);
impl ProbeTimer {
    fn new() -> windows::core::Result<Self> {
        use windows::Win32::System::Threading::*;
        unsafe {
            CreateWaitableTimerExW(
                None,
                windows::core::PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        }
        .map(Self)
    }
    fn wait(&self) -> windows::core::Result<()> {
        use windows::Win32::System::Threading::*;
        unsafe {
            SetWaitableTimerEx(self.0, &-5_000i64, 0, None, None, None, 0)?;
            WaitForSingleObject(self.0, 1000);
        }
        Ok(())
    }
}
impl Drop for ProbeTimer {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
