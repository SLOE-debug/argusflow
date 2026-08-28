//! 短期内存中的拥有型像素图片与捕获帧。

use std::{fmt, sync::Arc};

use argusflow_core::WindowIdentity;

use crate::{
    error::VisionError,
    frame::{FrameId, PhysicalRect, PixelFormat, QpcTimestamp, TopologyGeneration},
};

/// 短期内存中的拥有型像素图片；不实现序列化，防止意外进入 evidence。
#[derive(Clone)]
pub struct PixelImage {
    /// 图片宽度。
    pub width: u32,
    /// 图片高度。
    pub height: u32,
    /// 每行字节步长。
    pub stride_bytes: usize,
    /// 图片像素格式。
    pub format: PixelFormat,
    /// 图片像素，仅由图片所有者持有。
    pixels: Arc<[u8]>,
}

impl fmt::Debug for PixelImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PixelImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride_bytes", &self.stride_bytes)
            .field("format", &self.format)
            .field("byte_len", &self.pixels.len())
            .finish()
    }
}

impl PixelImage {
    /// 用已经校验过的像素创建图片。
    pub fn new(
        width: u32,
        height: u32,
        stride_bytes: usize,
        format: PixelFormat,
        pixels: impl Into<Arc<[u8]>>,
    ) -> Result<Self, VisionError> {
        if width == 0 || height == 0 {
            return Err(VisionError::InvalidFrame {
                message: "pixel image dimensions must be non-zero".to_owned(),
            });
        }
        let minimum_stride = width as usize * format.bytes_per_pixel();
        if stride_bytes < minimum_stride {
            return Err(VisionError::InvalidFrame {
                message: format!("stride {stride_bytes} is smaller than {minimum_stride}"),
            });
        }
        let pixels = pixels.into();
        let required_len =
            stride_bytes
                .checked_mul(height as usize)
                .ok_or_else(|| VisionError::InvalidFrame {
                    message: "pixel image byte length overflow".to_owned(),
                })?;
        if pixels.len() < required_len {
            return Err(VisionError::InvalidFrame {
                message: format!(
                    "pixel buffer has {} bytes, requires {required_len}",
                    pixels.len()
                ),
            });
        }
        Ok(Self {
            width,
            height,
            stride_bytes,
            format,
            pixels,
        })
    }

    /// 返回只读像素切片；调用方不得把内容持久化为默认行为。
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// 一张经过格式、尺寸和窗口身份校验的捕获帧。
#[derive(Clone)]
pub struct CapturedFrame {
    /// 捕获流内的单调帧 ID。
    pub frame_id: FrameId,
    /// 产生该帧时的窗口拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// 该帧对应的 HWND/PID 身份。
    pub window: WindowIdentity,
    /// 捕获实现提供的 QPC 刻度。
    pub timestamp_qpc: QpcTimestamp,
    /// 帧宽度。
    pub width: u32,
    /// 帧高度。
    pub height: u32,
    /// 水平 DPI。
    pub dpi_x: u32,
    /// 垂直 DPI。
    pub dpi_y: u32,
    /// 帧像素格式。
    pub pixel_format: PixelFormat,
    /// 客户区内容在帧中的位置。
    pub content_rect: PhysicalRect,
    /// 每行字节步长。
    pub stride_bytes: usize,
    /// 短期内存像素存储。
    storage: Arc<[u8]>,
}

impl fmt::Debug for CapturedFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapturedFrame")
            .field("frame_id", &self.frame_id)
            .field("topology_generation", &self.topology_generation)
            .field("window", &self.window)
            .field("timestamp_qpc", &self.timestamp_qpc)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("dpi_x", &self.dpi_x)
            .field("dpi_y", &self.dpi_y)
            .field("pixel_format", &self.pixel_format)
            .field("content_rect", &self.content_rect)
            .field("stride_bytes", &self.stride_bytes)
            .field("byte_len", &self.storage.len())
            .finish()
    }
}

impl CapturedFrame {
    /// 用 BGRA8 像素创建一张帧，适合 Windows readback 和 golden test。
    pub fn from_bgra8(
        frame_id: FrameId,
        topology_generation: TopologyGeneration,
        window: WindowIdentity,
        timestamp_qpc: QpcTimestamp,
        width: u32,
        height: u32,
        dpi_x: u32,
        dpi_y: u32,
        stride_bytes: usize,
        pixels: impl Into<Arc<[u8]>>,
    ) -> Result<Self, VisionError> {
        let content_rect =
            PhysicalRect::new(0, 0, width, height).ok_or_else(|| VisionError::InvalidFrame {
                message: "captured frame dimensions must be non-zero".to_owned(),
            })?;
        let pixels = pixels.into();
        let image = PixelImage::new(
            width,
            height,
            stride_bytes,
            PixelFormat::Bgra8Unorm,
            pixels.clone(),
        )?;
        Ok(Self {
            frame_id,
            topology_generation,
            window,
            timestamp_qpc,
            width: image.width,
            height: image.height,
            dpi_x: dpi_x.max(1),
            dpi_y: dpi_y.max(1),
            pixel_format: image.format,
            content_rect,
            stride_bytes: image.stride_bytes,
            storage: pixels,
        })
    }

    /// 返回不带复制的完整像素切片。
    pub fn pixels(&self) -> &[u8] {
        &self.storage
    }

    /// 返回帧的边界矩形。
    pub fn bounds(&self) -> PhysicalRect {
        self.content_rect
    }

    /// 读取一个帧内像素的 BGRA 四元组。
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = y as usize * self.stride_bytes + x as usize * 4;
        let pixel = self.storage.get(offset..offset + 4)?;
        Some([pixel[0], pixel[1], pixel[2], pixel[3]])
    }

    /// 从当前帧复制一个 ROI，为 OCR 或调试传输建立独立图片所有权。
    pub fn crop(&self, roi: PhysicalRect) -> Result<PixelImage, VisionError> {
        if !roi.is_inside(self.bounds()) {
            return Err(VisionError::InvalidRoi {
                rect: roi,
                frame_id: self.frame_id,
            });
        }
        let row_bytes = roi.width as usize * self.pixel_format.bytes_per_pixel();
        let mut pixels = vec![0_u8; row_bytes * roi.height as usize];
        for row in 0..roi.height as usize {
            let source_offset = (roi.y as usize + row) * self.stride_bytes + roi.x as usize * 4;
            let target_offset = row * row_bytes;
            pixels[target_offset..target_offset + row_bytes]
                .copy_from_slice(&self.storage[source_offset..source_offset + row_bytes]);
        }
        PixelImage::new(roi.width, roi.height, row_bytes, self.pixel_format, pixels)
    }
}
