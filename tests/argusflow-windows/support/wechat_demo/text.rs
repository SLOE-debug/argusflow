//! OCR 块统一投影为窗口坐标，快照查询不依赖具体推理后端。
use argusflow_capture_contracts::PixelRect;
use argusflow_core::ImagePoint;
use argusflow_vision::OcrResult;
use std::error::Error;

/// 已合并到全窗口坐标中的文本块。
#[derive(Clone, Debug)]
pub struct TextRecord {
    /// 原始文字，保持模型输出。
    pub text: String,
    /// 模型置信度。
    pub confidence: f32,
    /// 全窗口局部像素包围框。
    pub rect: PixelRect,
    /// 全窗口局部像素四边形。
    pub polygon: [ImagePoint; 4],
}
/// 将小图识别结果平移到全窗口，不直接使用小图坐标点击屏幕。
pub fn project(result: &OcrResult, region: PixelRect) -> Result<Vec<TextRecord>, Box<dyn Error>> {
    result
        .blocks()
        .iter()
        .map(|block| {
            let polygon = block.polygon().map(|point| ImagePoint {
                x: point.x + region.x() as f32,
                y: point.y + region.y() as f32,
            });
            let x = polygon
                .iter()
                .map(|p| p.x)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(region.x() as f32) as u32;
            let y = polygon
                .iter()
                .map(|p| p.y)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(region.y() as f32) as u32;
            let right = polygon
                .iter()
                .map(|p| p.x)
                .fold(0.0, f32::max)
                .ceil()
                .min(region.right() as f32) as u32;
            let bottom = polygon
                .iter()
                .map(|p| p.y)
                .fold(0.0, f32::max)
                .ceil()
                .min(region.bottom() as f32) as u32;
            Ok(TextRecord {
                text: block.text().into(),
                confidence: block.confidence(),
                rect: PixelRect::new(
                    x,
                    y,
                    right.checked_sub(x).ok_or("OCR 横坐标无效")?,
                    bottom.checked_sub(y).ok_or("OCR 纵坐标无效")?,
                )?,
                polygon,
            })
        })
        .collect()
}
