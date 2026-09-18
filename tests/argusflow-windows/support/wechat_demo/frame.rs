//! 全窗口不可变像素帧和局部裁剪，不包含 OCR 或微信语义。
use argusflow_capture_contracts::{PixelRect, ScreenRect};
use std::{error::Error, sync::Arc};

/// 紧密 BGR 三通道；忽略截图 BGRX 的无意义第四通道，避免虚假差分。
#[derive(Clone)]
pub struct Frame {
    /// 采集时的物理屏幕区域。
    pub bounds: ScreenRect,
    /// 宽×高×3 个不可变像素字节。
    pub pixels: Arc<Vec<u8>>,
}
impl Frame {
    /// 校验四通道输入长度，转换一次后供差分和 OCR 共同使用。
    pub fn from_bgrx(bounds: ScreenRect, bytes: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        if bytes.len() as u64 != u64::from(bounds.width()) * u64::from(bounds.height()) * 4 {
            return Err("窗口像素长度不匹配".into());
        }
        let pixels = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| p[..3].iter().copied())
            .collect();
        Ok(Self {
            bounds,
            pixels: Arc::new(pixels),
        })
    }
    /// 返回原分辨率小图；坐标始终相对于整个窗口。
    pub fn crop(&self, region: PixelRect) -> Result<Vec<u8>, Box<dyn Error>> {
        if !self.bounds.local().contains(region) {
            return Err("局部区域超出窗口".into());
        }
        let mut bytes = Vec::with_capacity(region.width() as usize * region.height() as usize * 3);
        for row in region.y()..region.bottom() {
            let start = (row as usize * self.bounds.width() as usize + region.x() as usize) * 3;
            bytes.extend_from_slice(&self.pixels[start..start + region.width() as usize * 3]);
        }
        Ok(bytes)
    }
}

/// 矩形并集包围框；输入已通过 PixelRect 构造校验。
pub fn union(
    a: PixelRect,
    b: PixelRect,
) -> Result<PixelRect, argusflow_capture_contracts::CaptureError> {
    let x = a.x().min(b.x());
    let y = a.y().min(b.y());
    // 不增加原始边界，仍保留构造器的错误传播。
    PixelRect::new(
        x,
        y,
        a.right().max(b.right()) - x,
        a.bottom().max(b.bottom()) - y,
    )
}
