//! 经校验的借用图像视图；三通道也可表示视频的 Y/Cb/Cr。
use argusflow_capture_contracts::{
    CaptureError, CaptureResult, PixelFormat, PixelImage, PixelRect,
};
use argusflow_core::FailureKind;

/// 同一比较中的三通道必须具有相同语义；四通道按白底合成后比较 RGB。
#[derive(Clone, Copy)]
pub struct ImageView<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) stride: usize,
    pub(crate) channels: usize,
    pub(crate) format: Option<PixelFormat>,
    pub(crate) region: PixelRect,
}
impl<'a> ImageView<'a> {
    /// 紧密三通道图像，不分配或复制；拒绝空尺寸、错误长度和超过 6400 万像素。
    pub fn triples(width: u32, height: u32, bytes: &'a [u8]) -> CaptureResult<Self> {
        let region = PixelRect::new(0, 0, width, height)?;
        if u64::from(width) * u64::from(height) > 64_000_000
            || u64::from(width) * u64::from(height) * 3 != bytes.len() as u64
        {
            return Err(invalid("三通道图像长度或尺寸无效"));
        }
        Ok(Self {
            bytes,
            stride: width as usize * 3,
            channels: 3,
            format: None,
            region,
        })
    }
    /// 读取已有共享像素中的完整指定区域，忽略行尾填充及 BGRX 的 X 通道。
    pub fn region(image: &'a PixelImage, region: PixelRect) -> CaptureResult<Self> {
        if !PixelRect::new(0, 0, image.width(), image.height())?.contains(region)
            || u64::from(region.width()) * u64::from(region.height()) > 64_000_000
        {
            return Err(invalid("比较区域超出图像或像素预算"));
        }
        Ok(Self {
            bytes: image.bytes(),
            stride: image.stride(),
            channels: 4,
            format: Some(image.format()),
            region,
        })
    }
    /// 视图宽度。
    pub fn width(self) -> u32 {
        self.region.width()
    }
    /// 视图高度。
    pub fn height(self) -> u32 {
        self.region.height()
    }
    pub(crate) fn row_chunk(self, y: usize, start: usize, end: usize) -> &'a [u8] {
        let offset =
            (y + self.region.y() as usize) * self.stride + self.region.x() as usize * self.channels;
        &self.bytes[offset + start * self.channels..offset + end * self.channels]
    }
    pub(crate) fn pixel(self, index: usize) -> [u8; 3] {
        let x = index % self.width() as usize + self.region.x() as usize;
        let y = index / self.width() as usize + self.region.y() as usize;
        let pixel = &self.bytes[y * self.stride + x * self.channels..];
        let (r, b) = if matches!(self.format, Some(PixelFormat::Bgrx8 | PixelFormat::Bgra8)) {
            (pixel[2], pixel[0])
        } else {
            (pixel[0], pixel[2])
        };
        let alpha = if matches!(self.format, Some(PixelFormat::Bgra8 | PixelFormat::Rgba8)) {
            u32::from(pixel[3])
        } else {
            255
        };
        [r, pixel[1], b]
            .map(|value| ((u32::from(value) * alpha + 255 * (255 - alpha) + 127) / 255) as u8)
    }
}
pub(crate) fn invalid(message: &str) -> CaptureError {
    CaptureError::new(FailureKind::InvalidInput, "image_difference", message)
}
