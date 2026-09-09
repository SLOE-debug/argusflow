//! 两个固定版本按区域逐批 GPU 比较，结果映射回屏幕物理像素。
use crate::capture::{
    gpu::{Graphics, PendingDifference, Texture, TileMap},
    pixels::GpuPixels,
    topology,
};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::oneshot;
struct Batch {
    pending: PendingDifference,
    _before: Arc<Texture>,
    _after: Arc<Texture>,
    region: PixelRect,
}
pub(super) struct CompareJob {
    pixels: Arc<GpuPixels>,
    before: Arc<TileMap>,
    after: Arc<TileMap>,
    regions: VecDeque<PixelRect>,
    operation: Operation,
    batch: Option<Batch>,
    result: PixelChanges,
    pub reply: Option<oneshot::Sender<CaptureResult<PixelChanges>>>,
}
impl CompareJob {
    pub fn new(
        pixels: Arc<GpuPixels>,
        before: Arc<TileMap>,
        regions: Vec<PixelRect>,
        operation: Operation,
        graphics: &Graphics,
    ) -> CaptureResult<Self> {
        pixels.check(&operation)?;
        let after = pixels.map.get()?;
        let mut split = VecDeque::new();
        for region in regions {
            let raw = topology::to_raw(region, after.width, after.height, pixels.rotation)?;
            // 每批最多 1024x1024，临时纹理不随整个桌面尺寸增长。
            for y in (raw.y()..raw.bottom()).step_by(1024) {
                for x in (raw.x()..raw.right()).step_by(1024) {
                    split.push_back(PixelRect::new(
                        x,
                        y,
                        1024.min(raw.right() - x),
                        1024.min(raw.bottom() - y),
                    )?);
                }
            }
            if split.len() > 8192 {
                return Err(CaptureError::new(
                    FailureKind::ResourceLimit,
                    "gpu_compare",
                    "比较批次数超限",
                ));
            }
        }
        let mut job = Self {
            pixels,
            before,
            after,
            regions: split,
            operation,
            batch: None,
            result: PixelChanges::default(),
            reply: None,
        };
        job.submit(graphics)?;
        Ok(job)
    }
    fn submit(&mut self, graphics: &Graphics) -> CaptureResult<()> {
        if let Some(region) = self.regions.pop_front() {
            let before = self.before.crop(graphics, region)?;
            let after = self.after.crop(graphics, region)?;
            let mut locations = Vec::new();
            for y in (0..region.height()).step_by(32) {
                for x in (0..region.width()).step_by(32) {
                    locations.push([x, y, x, y]);
                }
            }
            let pending = graphics
                .difference
                .submit(graphics, &before, &after, &locations)?;
            self.batch = Some(Batch {
                pending,
                _before: before,
                _after: after,
                region,
            });
        }
        Ok(())
    }
    pub fn poll(
        &mut self,
        graphics: &Graphics,
        config: &BackendConfig,
        stats: &mut CaptureStats,
    ) -> CaptureResult<bool> {
        if let Err(error) = self.pixels.check(&self.operation) {
            if let Some(reply) = self.reply.take() {
                let _ = reply.send(Err(error));
            }
            self.regions.clear();
        }
        if let Some(batch) = &self.batch {
            if batch.pending.started.elapsed() > config.gpu_timeout {
                return Err(CaptureError::new(
                    FailureKind::Timeout,
                    "gpu_compare",
                    "GPU 差分超时",
                ));
            }
            let Some((changes, _)) = batch.pending.poll(graphics)? else {
                return Ok(false);
            };
            stats.metadata_readback_bytes += batch.pending.bytes();
            self.result.changed_pixels += changes.changed_pixels;
            self.result.compared_pixels += changes.compared_pixels;
            for rect in changes.regions {
                let raw = PixelRect::new(
                    batch.region.x() + rect.x(),
                    batch.region.y() + rect.y(),
                    rect.width(),
                    rect.height(),
                )?;
                self.result.regions.push(topology::to_logical(
                    raw,
                    self.after.width,
                    self.after.height,
                    self.pixels.rotation,
                )?);
            }
            self.batch = None;
        }
        if !self.regions.is_empty() {
            match self.submit(graphics) {
                Ok(()) => {
                    unsafe { graphics.context.Flush() };
                    return Ok(false);
                }
                Err(error) => {
                    if let Some(reply) = self.reply.take() {
                        let _ = reply.send(Err(error));
                    }
                    return Ok(true);
                }
            }
        }
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(Ok(std::mem::take(&mut self.result)));
        }
        Ok(true)
    }
}
