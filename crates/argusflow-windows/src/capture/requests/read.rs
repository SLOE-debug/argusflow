//! 区域读回之后才在 CPU 旋转，输出坐标始终采用屏幕方向。
use crate::capture::{
    gpu::{Graphics, PendingRead, Texture},
    pixels::GpuPixels,
    topology,
};
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use std::sync::Arc;
use tokio::sync::oneshot;
pub(super) struct ReadJob {
    pixels: Arc<GpuPixels>,
    operation: Operation,
    pending: PendingRead,
    pub reply: Option<oneshot::Sender<CaptureResult<PixelImage>>>,
}
impl ReadJob {
    pub fn new(
        pixels: Arc<GpuPixels>,
        region: PixelRect,
        operation: Operation,
        graphics: &Graphics,
        reused: Option<Arc<Texture>>,
    ) -> CaptureResult<Self> {
        pixels.check(&operation)?;
        let map = pixels.map.get()?;
        let raw = topology::to_raw(region, map.width, map.height, pixels.rotation)?;
        let source = map.crop(graphics, raw)?;
        let pending = PendingRead::new(graphics, source, reused)?;
        Ok(Self {
            pixels,
            operation,
            pending,
            reply: None,
        })
    }
    pub fn poll(
        &mut self,
        graphics: &Graphics,
        cpu: &ByteBudget,
        config: &BackendConfig,
        stats: &mut CaptureStats,
        pool: &mut Vec<Arc<Texture>>,
    ) -> CaptureResult<bool> {
        if let Err(error) = self.pixels.check(&self.operation)
            && let Some(reply) = self.reply.take()
        {
            let _ = reply.send(Err(error));
        }
        if self.pending.started.elapsed() > config.gpu_timeout {
            return Err(CaptureError::new(
                FailureKind::Timeout,
                "gpu_read",
                "GPU 读回超时",
            ));
        }
        match self.pending.poll(graphics, cpu) {
            Ok(Some(image)) => {
                stats.pixel_readback_bytes +=
                    u64::from(image.width()) * u64::from(image.height()) * 4;
                let result = rotate(image, self.pixels.rotation, cpu);
                if let Some(reply) = self.reply.take() {
                    let _ = reply.send(result);
                }
                pool.clear();
                pool.push(self.pending.staging.clone());
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(error) if error.kind() == FailureKind::ResourceLimit => {
                if let Some(reply) = self.reply.take() {
                    let _ = reply.send(Err(error));
                }
                Ok(true)
            }
            Err(error) => Err(error),
        }
    }
}
fn rotate(image: PixelImage, rotation: Rotation, cpu: &ByteBudget) -> CaptureResult<PixelImage> {
    if rotation == Rotation::Identity {
        return Ok(image);
    }
    let (width, height) = match rotation {
        Rotation::Clockwise90 | Rotation::Clockwise270 => (image.height(), image.width()),
        _ => (image.width(), image.height()),
    };
    let reservation = cpu.reserve(width as usize * height as usize * 4)?;
    let mut bytes = vec![0; width as usize * height as usize * 4];
    for y in 0..image.height() {
        for x in 0..image.width() {
            let (tx, ty) = match rotation {
                Rotation::Identity => (x, y),
                Rotation::Clockwise90 => (image.height() - 1 - y, x),
                Rotation::Clockwise180 => (image.width() - 1 - x, image.height() - 1 - y),
                Rotation::Clockwise270 => (y, image.width() - 1 - x),
            };
            let source = y as usize * image.stride() + x as usize * 4;
            let target = (ty as usize * width as usize + tx as usize) * 4;
            bytes[target..target + 4].copy_from_slice(&image.bytes()[source..source + 4]);
        }
    }
    PixelImage::new(
        width,
        height,
        width as usize * 4,
        PixelFormat::Bgrx8,
        bytes,
        reservation,
    )
}

#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/read.rs"]
mod tests;
