//! 单显示器的有序区域读回及真实呈现元数据。

use super::{
    desktop_device::DesktopDevice,
    desktop_output::DesktopOutput,
    desktop_pixels::{OutputCrop, Rotation},
    readback_queue::ReadbackQueue,
};
use argusflow_core::{
    CaptureGeneration, CaptureSourceId, CaptureTiming, EvidenceFrame, EvidencePixelFormat,
    InspectionFailure, InspectionRect, capture::ScreenCaptureUpdate,
};
use windows::Win32::{Foundation::RECT, Graphics::Direct3D11::D3D11_BOX};

pub(super) struct UpdateMetadata {
    bounds: RECT,
    rotation: Rotation,
    source: CaptureSourceId,
    generation: CaptureGeneration,
    presented_us: u64,
    accumulated: u32,
    reset: bool,
    regions: Vec<InspectionRect>,
}

/// 独立于同步截图的流式状态，每个输出只创建一次。
#[derive(Default)]
pub(super) struct DesktopUpdateQueue {
    readback: ReadbackQueue<UpdateMetadata>,
    initialized: bool,
}

impl DesktopUpdateQueue {
    /// 新订阅从当前像素建立基准，不把上一订阅的未交付增量当作首帧。
    pub(super) fn request_baseline(&mut self) {
        self.initialized = false;
        self.readback = ReadbackQueue::default();
    }
    pub(super) fn pending(&self) -> bool {
        self.readback.pending()
    }
    pub(super) fn poll(
        &mut self,
        capture: &mut DesktopOutput,
        graphics: &DesktopDevice,
        bounds: RECT,
        rotation: Rotation,
        source: CaptureSourceId,
        generation: CaptureGeneration,
        acquire: bool,
    ) -> Result<Option<ScreenCaptureUpdate>, InspectionFailure> {
        let complete = self
            .readback
            .poll(&graphics.context)?
            .map(|(metadata, pixels)| {
                let mut patches = Vec::with_capacity(pixels.len());
                for (pixels, region) in pixels.into_iter().zip(&metadata.regions) {
                    let crop = OutputCrop::new(*region, metadata.bounds, metadata.rotation)
                        .ok_or(InspectionFailure::InvalidGeometry)?;
                    let width = region.width as u32;
                    let height = region.height as u32;
                    let mut output = vec![0; width as usize * height as usize * 4];
                    let texture_width = pixels.region.right - pixels.region.left;
                    let texture_height = pixels.region.bottom - pixels.region.top;
                    crop.copy_cropped(
                        &pixels.pixels,
                        texture_width as usize * 4,
                        texture_width,
                        texture_height,
                        &mut output,
                    )?;
                    patches.push(EvidenceFrame::new(
                        *region,
                        width,
                        height,
                        EvidencePixelFormat::Bgrx8,
                        output,
                    )?);
                }
                Ok(ScreenCaptureUpdate {
                    source: metadata.source,
                    generation: metadata.generation,
                    bounds: screen_bounds(metadata.bounds),
                    timing: CaptureTiming {
                        presented_us: metadata.presented_us,
                        frozen_us: super::clock::now_us()?,
                    },
                    reset: metadata.reset,
                    accumulated_frames: metadata.accumulated,
                    patches,
                })
            })
            .transpose()?;
        if acquire && self.readback.available() {
            let changed = match capture.refresh_now(graphics) {
                Ok(changed) => changed,
                Err(InspectionFailure::Timeout) if !self.initialized => false,
                Err(error) => return Err(error),
            };
            if changed || (!self.initialized && capture.texture().is_ok()) {
                let reset = !self.initialized;
                let regions =
                    if reset || capture.damage().is_empty() || capture.damage().len() > 128 {
                        vec![screen_bounds(bounds)]
                    } else {
                        capture
                            .damage()
                            .iter()
                            .map(|rect| transform(*rect, bounds, rotation))
                            .collect::<Result<Vec<_>, _>>()?
                    };
                let copies: Vec<D3D11_BOX> = regions
                    .iter()
                    .map(|region| {
                        OutputCrop::new(*region, bounds, rotation)
                            .map(|crop| crop.texture_region())
                            .ok_or(InspectionFailure::InvalidGeometry)
                    })
                    .collect::<Result<_, _>>()?;
                self.readback.submit(
                    &graphics.device,
                    &graphics.context,
                    capture.texture()?,
                    copies,
                    UpdateMetadata {
                        bounds,
                        rotation,
                        source,
                        generation,
                        presented_us: super::clock::qpc_us(capture.presented_qpc())?,
                        accumulated: if reset {
                            1
                        } else {
                            capture.accumulated_frames()
                        },
                        reset,
                        regions,
                    },
                )?;
                self.initialized = true;
            }
        }
        Ok(complete)
    }
}

fn screen_bounds(bounds: RECT) -> InspectionRect {
    InspectionRect {
        x: bounds.left.into(),
        y: bounds.top.into(),
        width: f64::from(bounds.right) - f64::from(bounds.left),
        height: f64::from(bounds.bottom) - f64::from(bounds.top),
    }
}

fn transform(
    rect: RECT,
    output: RECT,
    rotation: Rotation,
) -> Result<InspectionRect, InspectionFailure> {
    let width = output.right - output.left;
    let height = output.bottom - output.top;
    let (left, top, right, bottom) = match rotation {
        Rotation::Identity => (rect.left, rect.top, rect.right, rect.bottom),
        Rotation::Clockwise90 => (width - rect.bottom, rect.left, width - rect.top, rect.right),
        Rotation::Clockwise180 => (
            width - rect.right,
            height - rect.bottom,
            width - rect.left,
            height - rect.top,
        ),
        Rotation::Clockwise270 => (
            rect.top,
            height - rect.right,
            rect.bottom,
            height - rect.left,
        ),
    };
    if left < 0 || top < 0 || right > width || bottom > height || right <= left || bottom <= top {
        return Err(InspectionFailure::InvalidGeometry);
    }
    Ok(InspectionRect {
        x: f64::from(output.left) + f64::from(left),
        y: f64::from(output.top) + f64::from(top),
        width: (right - left).into(),
        height: (bottom - top).into(),
    })
}
