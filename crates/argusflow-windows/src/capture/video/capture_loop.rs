//! 持续桌面采集；静态心跳推动封装落盘，真实呈现时间永不被心跳覆盖。
use super::{
    graphics::Graphics,
    journal::{FrameEntry, Journal, Message, PendingFrame},
    model::{Result, VideoError, VideoOptions, VideoReport, required},
    storage::StorageGuard,
    timing::{pts, qpc},
};
use crate::capture::clock::Clock;
use std::{
    sync::mpsc::{Receiver, SyncSender},
    time::{Duration, Instant},
};
use windows::{
    Win32::Graphics::{Direct3D11::ID3D11Texture2D, Dxgi::*},
    core::Interface,
};
struct FrameLease<'a>(&'a IDXGIOutputDuplication);
impl Drop for FrameLease<'_> {
    fn drop(&mut self) {
        // SAFETY: 仅在Acquire成功后构造，所有分支都恰好释放一次。
        let _ = unsafe { self.0.ReleaseFrame() };
    }
}
pub(super) struct CaptureLoop<'a> {
    pub graphics: &'a Graphics,
    pub duplication: &'a IDXGIOutputDuplication,
    pub available: &'a Receiver<ID3D11Texture2D>,
    pub sender: &'a SyncSender<Message>,
    pub gaps: &'a mut Journal,
    pub clock: Clock,
    pub options: &'a VideoOptions,
    pub report: &'a mut VideoReport,
    pub stop: &'a std::sync::atomic::AtomicBool,
}
impl CaptureLoop<'_> {
    pub fn run(&mut self) -> Result<()> {
        let started = Instant::now();
        let mut last = 0i64;
        let mut last_sample = Instant::now();
        let interval = Duration::from_secs_f64(1.0 / f64::from(self.options.fps));
        let cached = self.graphics.texture()?;
        let mut storage = StorageGuard::new(&self.options.directory, self.options.max_file_bytes)?;
        let mut overload = None;
        while started.elapsed() < self.options.duration
            && !self.stop.load(std::sync::atomic::Ordering::Acquire)
        {
            storage.tick()?;
            // 按采集时钟限速，不把刚取得的末尾更新丢掉；下一次Acquire获取最新桌面。
            if self.report.frames > 0 && last_sample.elapsed() < interval {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;
            // SAFETY: 短等待保证定期处理停止和磁盘检查，不依赖桌面必须变化。
            let acquired_frame = match unsafe {
                self.duplication
                    .AcquireNextFrame(10, &mut info, &mut resource)
            } {
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => false,
                Err(e) => return Err(e.into()),
                Ok(()) => true,
            };
            let _lease = acquired_frame.then(|| FrameLease(self.duplication));
            let acquired = qpc()?;
            if info.ProtectedContentMaskedOut.as_bool() {
                return Err(VideoError::Invalid("受保护画面被遮蔽".into()));
            }
            let changed = acquired_frame && (info.LastPresentTime != 0 || self.report.frames == 0);
            let heartbeat = !changed
                && self.report.frames > 0
                && last_sample.elapsed() >= Duration::from_secs(1);
            if !changed && !heartbeat {
                continue;
            }
            let presented = if heartbeat {
                last
            } else if info.LastPresentTime == 0 {
                acquired
            } else {
                info.LastPresentTime
            };
            self.report.accumulated_updates += u64::from(info.AccumulatedFrames.saturating_sub(1));
            let texture = match self.available.try_recv() {
                Ok(texture) => texture,
                Err(_) => {
                    self.report.pool_drops += 1;
                    self.gaps.write(&serde_json::json!({"presented_qpc":presented,"observed_qpc":acquired,"reason":"texture_pool_exhausted","accumulated":info.AccumulatedFrames}))?;
                    let since = overload.get_or_insert_with(Instant::now);
                    if since.elapsed() >= Duration::from_secs(2) {
                        return Err(VideoError::Invalid(
                            "编码连续2秒未释放纹理，停止采集".into(),
                        ));
                    }
                    continue;
                }
            };
            overload = None;
            if heartbeat {
                self.graphics.copy(&cached, &texture);
            } else {
                let input: ID3D11Texture2D = required(resource, "DXGI resource")?.cast()?;
                self.graphics.convert(&input, &texture)?;
                self.graphics.copy(&texture, &cached);
            }
            let sample_pts = if self.report.frames == 0 {
                0
            } else {
                pts(if heartbeat { acquired } else { presented }, self.clock)
            };
            let entry = FrameEntry {
                sequence: self.report.frames + 1,
                presented_qpc: presented,
                acquired_qpc: acquired,
                pts_100ns: super::timing::media_pts(sample_pts),
                accumulated: info.AccumulatedFrames,
                repeated: heartbeat,
            };
            // 所有帧都占用池槽，队列不能越过固定GPU预算。
            self.sender
                .send(Message::Frame(PendingFrame { texture, entry }))
                .map_err(|_| VideoError::Invalid("编码线程已经停止".into()))?;
            last = presented;
            last_sample = Instant::now();
            self.report.frames += 1;
            if self.report.frames == 1 {
                let mut ready = Journal::new(&self.options.directory.join("ready.json"))?;
                ready.write(&serde_json::json!({"first_qpc":acquired}))?;
                ready.sync()?;
            }
        }
        if self.report.frames == 0 {
            return Err(VideoError::Invalid("没有采到任何桌面帧".into()));
        }
        Ok(())
    }
}
