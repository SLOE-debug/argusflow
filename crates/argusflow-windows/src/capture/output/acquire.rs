//! 有限等待 Acquire，复制当前最终像素后立即释放 DXGI 帧租约。
use super::{
    damage,
    state::{Output, PendingFrame},
};
use crate::capture::gpu::{
    Completion, Graphics, Texture, TileBatch, TileMap, failure, invalid, tiles_for_regions,
};
use argusflow_capture_contracts::*;
use std::sync::Arc;
use windows::{
    Win32::Graphics::{
        Direct3D11::*,
        Dxgi::{Common::DXGI_FORMAT_B8G8R8A8_UNORM, *},
    },
    core::Interface,
};

struct Lease(IDXGIOutputDuplication);
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = unsafe { self.0.ReleaseFrame() };
    }
}
impl Output {
    pub fn acquire(
        &mut self,
        graphics: &Graphics,
        wait_ms: u32,
        stats: &mut CaptureStats,
    ) -> CaptureResult<()> {
        if self.pending.is_some() || self.baseline.is_some() {
            return Ok(());
        }
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        // 此前检测点作为健康水位，不能用 Acquire 返回后的未来时间越过竞态窗口。
        let checked = self.clock.now();
        match unsafe {
            self.duplication
                .AcquireNextFrame(wait_ms, &mut info, &mut resource)
        } {
            Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                if self.map.is_some() {
                    self.checked = checked;
                    self.watermark();
                }
                if self.pressure && self.desktop.is_some() {
                    self.establish(graphics, stats)?;
                }
                return Ok(());
            }
            Err(error) => return Err(failure(error)),
            Ok(()) => {}
        }
        let lease = Lease(self.duplication.clone());
        stats.acquired_frames += 1;
        if info.ProtectedContentMaskedOut.as_bool() {
            self.gap(GapReason::Protected);
            return Err(CaptureError::new(
                argusflow_core::FailureKind::Unavailable,
                "dxgi_protected",
                "桌面包含受保护内容",
            ));
        }
        if info.LastPresentTime == 0 && self.desktop.is_some() {
            if self.pressure {
                self.establish(graphics, stats)?;
            }
            self.checked = checked;
            self.watermark();
            return Ok(());
        }
        let source: ID3D11Texture2D = resource
            .ok_or_else(|| invalid("missing desktop resource"))?
            .cast()
            .map_err(failure)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut desc) };
        if desc.Width != self.width
            || desc.Height != self.height
            || desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || desc.SampleDesc.Count != 1
        {
            return Err(invalid("desktop texture format or topology changed"));
        }
        let acquired = self.clock.now();
        let presented =
            (info.LastPresentTime != 0).then(|| self.clock.convert(info.LastPresentTime));
        if info.AccumulatedFrames > 1 {
            self.queue.push(
                BackendEvent::Gap(Gap {
                    source: self.info.id,
                    from: self.last_present,
                    through: presented.unwrap_or(acquired),
                    reason: GapReason::Accumulated,
                }),
                acquired,
            );
            stats.gaps += 1;
        }
        let timing = Timing {
            presented,
            acquired,
            frozen: acquired,
        };
        if self.desktop.is_none() {
            let texture = Arc::new(Texture::new(graphics, self.width, self.height, false)?);
            unsafe { graphics.context.CopyResource(&texture.native, &source) };
            self.desktop = Some(texture);
            self.last_present = presented.unwrap_or(acquired);
            self.checked = checked;
            self.establish_with_timing(graphics, timing)?;
            unsafe { graphics.context.Flush() };
            drop(lease);
            return Ok(());
        }
        let regions = damage::read(
            &self.duplication,
            info.TotalMetadataBufferSize,
            self.width,
            self.height,
        )?;
        let desktop = self
            .desktop
            .as_ref()
            .ok_or_else(|| invalid("missing baseline"))?
            .clone();
        if let Some(map) = &self.map {
            let indices = tiles_for_regions(self.width, self.height, &regions)?;
            if !indices.is_empty() {
                // 在提交任何批次前检查完整的临时资源需求，避免半批提交后丢掉预算租约。
                let needed = indices
                    .chunks(2048)
                    .map(|batch| {
                        let count = batch.len();
                        let columns = count.min(32);
                        columns * 32 * count.div_ceil(columns) * 32 * 4 + count * 80 + 16
                    })
                    .sum::<usize>();
                if graphics.budget.used().saturating_add(needed) > graphics.budget.limit() {
                    // 即使历史预算耗尽，也先保持当前桌面完整；下一帧 dirty 只覆盖此后的变化。
                    unsafe {
                        graphics.context.CopyResource(&desktop.native, &source);
                        graphics.context.Flush();
                    }
                    self.last_present = presented.unwrap_or(acquired);
                    self.checked = checked;
                    return Err(CaptureError::new(
                        argusflow_core::FailureKind::ResourceLimit,
                        "desktop_history",
                        "GPU 历史预算不足",
                    ));
                }
                let mut batches = Vec::new();
                for indices in indices.chunks(2048) {
                    let batch =
                        TileBatch::freeze(graphics, &source, self.width, self.height, indices)?;
                    let difference = graphics.difference.submit(
                        graphics,
                        &desktop,
                        &batch.atlas,
                        &batch.locations,
                    )?;
                    batches.push((batch, difference));
                }
                self.pending = Some(PendingFrame {
                    batches,
                    timing,
                    changes: PixelChanges::default(),
                    next: map.clone(),
                    index: 0,
                    completion: None,
                });
            }
        }
        for region in regions {
            let bounds = D3D11_BOX {
                left: region.x(),
                top: region.y(),
                front: 0,
                right: region.right(),
                bottom: region.bottom(),
                back: 1,
            };
            // 移动目的区从最新桌面读取，避免同纹理重叠移动破坏旧像素。
            unsafe {
                graphics.context.CopySubresourceRegion(
                    &desktop.native,
                    0,
                    region.x(),
                    region.y(),
                    0,
                    &source,
                    0,
                    Some(&bounds),
                )
            };
        }
        self.last_present = presented.unwrap_or(acquired);
        self.checked = checked;
        if self.map.is_none() {
            self.establish_with_timing(graphics, timing)?;
        }
        unsafe { graphics.context.Flush() };
        drop(lease);
        Ok(())
    }
    fn establish(&mut self, graphics: &Graphics, _stats: &mut CaptureStats) -> CaptureResult<()> {
        self.establish_with_timing(
            graphics,
            Timing {
                presented: Some(self.last_present),
                acquired: self.clock.now(),
                frozen: self.clock.now(),
            },
        )?;
        unsafe { graphics.context.Flush() };
        Ok(())
    }
    fn establish_with_timing(&mut self, graphics: &Graphics, timing: Timing) -> CaptureResult<()> {
        let initial = Arc::new(Texture::new(graphics, self.width, self.height, false)?);
        let desktop = self
            .desktop
            .as_ref()
            .ok_or_else(|| invalid("missing desktop baseline"))?;
        unsafe {
            graphics
                .context
                .CopyResource(&initial.native, &desktop.native)
        };
        self.map = Some(TileMap::initial(initial));
        self.pressure = false;
        self.baseline = Some((Completion::submit(graphics)?, timing));
        Ok(())
    }
}
