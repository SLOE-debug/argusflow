//! 事件时刻像素采样契约，与 OCR、元素定位和执行目标构造无关。

use crate::{InspectionContext, InspectionFailure, InspectionRect};

/// 截图原生字节布局；GDI 的第四字节未定义，不能冒充透明度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePixelFormat {
    /// 红、绿、蓝、Alpha，四字节均有效。
    Rgba8,
    /// 蓝、绿、红、保留字节，显示及编码前须转换为不透明 RGBA。
    Bgrx8,
}

/// 冻结的自有像素；捕获线程可以保留原生布局，颜色转换交给编码线程。
#[derive(Clone)]
pub struct EvidenceFrame {
    /// 实际采集的屏幕物理范围。
    bounds: InspectionRect,
    /// 像素宽度，所有行紧密排列。
    width: u32,
    /// 像素高度，行从上到下排列。
    height: u32,
    /// 明确记录原生通道顺序与 Alpha 有效性。
    format: EvidencePixelFormat,
    /// 独立于后端可复用位图，后续截图不能覆盖已经冻结的证据。
    pixels: Vec<u8>,
}

impl EvidenceFrame {
    /// 像素行按从上到下排列，矩形使用屏幕物理坐标。
    pub fn new(
        bounds: InspectionRect,
        width: u32,
        height: u32,
        format: EvidencePixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, InspectionFailure> {
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if !bounds.is_valid() || width == 0 || height == 0 || length != Some(pixels.len()) {
            return Err(InspectionFailure::InvalidGeometry);
        }
        Ok(Self {
            bounds,
            width,
            height,
            format,
            pixels,
        })
    }

    /// 截图覆盖的实际屏幕范围，可能裁切屏幕外窗口边缘。
    pub fn bounds(&self) -> InspectionRect {
        self.bounds
    }
    /// 图像宽度，物理像素。
    pub fn width(&self) -> u32 {
        self.width
    }
    /// 图像高度，物理像素。
    pub fn height(&self) -> u32 {
        self.height
    }
    /// 原生通道布局，消费者必须按此解释像素。
    pub fn format(&self) -> EvidencePixelFormat {
        self.format
    }

    /// 只读原生像素，禁止调用方修改已冻结帧。
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// 消费并原地转换像素；仅由图像编码线程调用，不重新分配整帧缓冲。
    pub fn into_rgba8(mut self) -> Self {
        match self.format {
            EvidencePixelFormat::Rgba8 => {}
            EvidencePixelFormat::Bgrx8 => {
                for pixel in self.pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                    pixel[3] = 255;
                }
                self.format = EvidencePixelFormat::Rgba8;
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_pixels_are_converted_in_place_with_opaque_alpha() {
        let bounds = InspectionRect {
            x: -2.0,
            y: 1.0,
            width: 2.0,
            height: 1.0,
        };
        let frame = EvidenceFrame::new(
            bounds,
            2,
            1,
            EvidencePixelFormat::Bgrx8,
            vec![11, 22, 33, 0, 44, 55, 66, 127],
        )
        .unwrap();
        let allocation = frame.pixels().as_ptr();
        let rgba = frame.into_rgba8();
        assert_eq!(rgba.pixels(), &[33, 22, 11, 255, 66, 55, 44, 255]);
        assert_eq!(rgba.pixels().as_ptr(), allocation);
        assert_eq!(rgba.format(), EvidencePixelFormat::Rgba8);
        assert_eq!(rgba.into_rgba8().pixels().as_ptr(), allocation);
        assert!(EvidenceFrame::new(bounds, 2, 1, EvidencePixelFormat::Bgrx8, vec![0; 7]).is_err());
    }
}

/// 快速同步截图边界；采集发生在慢速 UIA/CDP 检查前，不调用 OCR。
pub trait WindowEvidenceCapture: Send + Sync {
    /// 保存用户当时可见的窗口区域；遮挡也属于观察事实，不承诺离屏窗口内容。
    fn capture(&self, context: &InspectionContext) -> Result<EvidenceFrame, InspectionFailure>;
}
