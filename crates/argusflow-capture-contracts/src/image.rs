//! 不可变原始像素，不包含编码或磁盘路径。
use crate::{CaptureError, CaptureResult, PixelRect, Reservation};
use argusflow_core::FailureKind;
use std::sync::Arc;

/// u8 四通道布局；BGRX 的第四通道不参与颜色或透明度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// B、G、R、无意义填充字节。
    Bgrx8,
    /// B、G、R、非预乘透明度。
    Bgra8,
    /// R、G、B、非预乘透明度。
    Rgba8,
}
struct ImageData {
    bytes: Vec<u8>,
    _reservation: Reservation,
}
/// 共享只读像素；克隆不复制图像，也不释放预算。
#[derive(Clone)]
pub struct PixelImage {
    width: u32,
    height: u32,
    stride: usize,
    format: PixelFormat,
    data: Arc<ImageData>,
}
impl PixelImage {
    /// 用覆盖实际缓冲的预留构造经过布局验证的像素。
    pub fn new(
        width: u32,
        height: u32,
        stride: usize,
        format: PixelFormat,
        bytes: Vec<u8>,
        reservation: Reservation,
    ) -> CaptureResult<Self> {
        PixelRect::new(0, 0, width, height)?;
        if stride < width as usize * 4
            || stride.checked_mul(height as usize) != Some(bytes.len())
            || reservation.bytes() < bytes.capacity()
        {
            return Err(CaptureError::new(
                FailureKind::InvalidInput,
                "pixel_image",
                "图像布局或预算不匹配",
            ));
        }
        Ok(Self {
            width,
            height,
            stride,
            format,
            data: Arc::new(ImageData {
                bytes,
                _reservation: reservation,
            }),
        })
    }
    /// 图像宽度。
    pub fn width(&self) -> u32 {
        self.width
    }
    /// 图像高度。
    pub fn height(&self) -> u32 {
        self.height
    }
    /// 每行字节数，包含行尾填充。
    pub fn stride(&self) -> usize {
        self.stride
    }
    /// 通道布局。
    pub fn format(&self) -> PixelFormat {
        self.format
    }
    /// 生命周期受共享所有权保护的只读字节。
    pub fn bytes(&self) -> &[u8] {
        &self.data.bytes
    }
}
impl std::fmt::Debug for PixelImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixelImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}
