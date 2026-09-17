//! 单显示器按上限帧率获取完整图像；合并刷新不是办公帧流的失败。
use super::service::Shared;
use crate::capture::{
    gpu::{Graphics, Scaler, failure, invalid},
    topology::OutputInfo,
};
use argusflow_capture_contracts::*;
use argusflow_core::FailureKind;
use std::time::{Duration, Instant};
use windows::{
    Win32::Graphics::{
        Direct3D11::*,
        Dxgi::{Common::DXGI_FORMAT_B8G8R8A8_UNORM, *},
    },
    core::Interface,
};

struct FrameLease(IDXGIOutputDuplication);
impl Drop for FrameLease {
    fn drop(&mut self) {
        let _ = unsafe { self.0.ReleaseFrame() };
    }
}
pub(super) struct Output {
    pub info: SourceInfo,
    duplication: IDXGIOutputDuplication,
    scaler: Scaler,
    raw: (u32, u32),
    pending: Option<(Timing, ClockTime, Instant)>,
    next: Instant,
    revision: u64,
}
impl Output {
    pub fn new(spec: OutputInfo, graphics: &Graphics, config: &FrameConfig) -> CaptureResult<Self> {
        let size = config.size(spec.info.bounds.width(), spec.info.bounds.height());
        let duplication =
            unsafe { spec.output.DuplicateOutput(&graphics.device) }.map_err(failure)?;
        Ok(Self {
            scaler: Scaler::new(
                graphics,
                (spec.raw_width, spec.raw_height),
                size,
                spec.info.rotation,
            )?,
            info: spec.info,
            duplication,
            raw: (spec.raw_width, spec.raw_height),
            pending: None,
            next: Instant::now(),
            revision: 0,
        })
    }
    pub fn poll(
        &mut self,
        graphics: &Graphics,
        shared: &Shared,
        config: &FrameConfig,
    ) -> CaptureResult<()> {
        if let Some((mut timing, checked, started)) = self.pending {
            shared
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .make_room(&shared.cpu, self.scaler.byte_len());
            // 已交给编码的帧不能回收；等待预算释放，不创建新设备或越过旧水位。
            if shared.cpu.used().saturating_add(self.scaler.byte_len()) > shared.cpu.limit() {
                if started.elapsed() > Duration::from_secs(2) {
                    return Err(CaptureError::new(
                        FailureKind::ResourceLimit,
                        "frame_cache",
                        "固定图片持续占满全屏缓存预算",
                    ));
                }
                return Ok(());
            }
            if started.elapsed() > Duration::from_secs(2) {
                return Err(CaptureError::new(
                    FailureKind::Timeout,
                    "frame_freeze",
                    "全屏帧读回未在2秒内完成",
                ));
            }
            let Some(image) = self.scaler.poll(graphics, &shared.cpu)? else {
                return Ok(());
            };
            timing.frozen = shared.clock.now();
            self.info.state = SourceState::Ready;
            self.info.failure = None;
            self.revision += 1;
            shared
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .publish(
                    DesktopFrame {
                        version: Version {
                            session: shared.clock.0.session,
                            source: self.info.id,
                            generation: self.info.generation,
                            revision: self.revision,
                        },
                        source: self.info.clone(),
                        timing,
                        image,
                    },
                    checked,
                    config.history,
                );
            self.pending = None;
        }
        if Instant::now() < self.next {
            return Ok(());
        }
        self.next = Instant::now() + Duration::from_secs_f64(1.0 / f64::from(config.fps));
        let checked = shared.clock.now();
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        match unsafe {
            self.duplication
                .AcquireNextFrame(0, &mut info, &mut resource)
        } {
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                shared
                    .store
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .checked(self.info.id, checked);
                return Ok(());
            }
            Err(e) => return Err(failure(e)),
            Ok(()) => {}
        }
        let _lease = FrameLease(self.duplication.clone());
        if info.ProtectedContentMaskedOut.as_bool() {
            return Err(CaptureError::new(
                FailureKind::Unavailable,
                "frame_capture",
                "屏幕包含受保护内容",
            ));
        }
        if info.LastPresentTime == 0 && self.revision > 0 {
            shared
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .checked(self.info.id, checked);
            return Ok(());
        }
        let source: ID3D11Texture2D = resource
            .ok_or_else(|| invalid("missing frame texture"))?
            .cast()
            .map_err(failure)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };
        if (desc.Width, desc.Height) != self.raw
            || desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || desc.SampleDesc.Count != 1
        {
            return Err(invalid("frame topology changed"));
        }
        let acquired = shared.clock.now();
        self.scaler.submit(graphics, &source)?;
        self.pending = Some((
            Timing {
                presented: (info.LastPresentTime != 0)
                    .then(|| shared.clock.convert(info.LastPresentTime)),
                acquired,
                frozen: acquired,
            },
            checked,
            Instant::now(),
        ));
        Ok(())
    }
}
