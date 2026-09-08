//! 事件像素的 PNG 持久化与点击局部裁切，不调用 Vision/OCR。

use argusflow_core::{
    EvidenceFrame, EvidencePixelFormat, InspectionFailure, InspectionRect, ScreenPoint,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

/// 读取已保存图像的封闭类型，调用方不能提供任意路径。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotKind {
    /// 点击时保留的目标画面。
    Target,
    /// 点击目标的局部图。
    TargetCrop,
    /// 完整可见窗口区域。
    Window,
    /// 点击附近局部区域。
    Crop,
}

/// 点击附近的局部证据，裁切范围使用完整帧的本地物理像素。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotCrop {
    /// 相对于演示包根目录的 PNG 路径。
    pub path: String,
    /// 在完整帧内的裁切范围。
    pub bounds: InspectionRect,
}

/// 图像证据有明确采样时间与坐标空间，不包含假想元素。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotEvidence {
    /// 操作后连续稳定；false 表示预算内仍有变化，不宣称视觉结果已完成。
    pub stabilized: bool,
    /// 点击框根据实际附近像素计算的 RGB 色；未点击时为空。
    pub click_color: Option<[u8; 3]>,
    /// 相对于演示包根目录的 PNG 路径。
    pub path: String,
    /// 像素采集开始时间，相对录制开始的毫秒数。
    pub captured_at_ms: u64,
    /// 采样耗时，毫秒；异步观察不宣称绝对原子性。
    pub capture_duration_ms: u64,
    /// 实际可见窗口区域在虚拟屏幕中的范围。
    pub screen_bounds: InspectionRect,
    /// 图像像素宽度。
    pub width: u32,
    /// 图像像素高度。
    pub height: u32,
    /// 鼠标事件的屏幕物理坐标。
    pub pointer: Option<ScreenPoint>,
    /// 按下位置附近的局部 PNG，允许只保存完整窗口。
    pub crop: Option<ScreenshotCrop>,
    /// 局部 PNG 保存失败时仍保留完整窗口图像，并明确指出缺失原因。
    pub crop_failure: Option<InspectionFailure>,
}

/// 每次录制独占目录，像素在操作后采样期间保存，停止后仅发布 JSON 清单。
pub(crate) struct ScreenshotStore {
    directory: PathBuf,
}

impl ScreenshotStore {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// 文件名只来自单调事件序号；保存失败显式报告证据缺失。
    pub(crate) fn save(
        &self,
        sequence: u64,
        frame: &EvidenceFrame,
        captured_at_ms: u64,
        capture_duration_ms: u64,
        pointer: Option<ScreenPoint>,
        crop_click: bool,
    ) -> Result<ScreenshotEvidence, InspectionFailure> {
        self.save_named(
            &sequence.to_string(),
            frame,
            captured_at_ms,
            capture_duration_ms,
            pointer,
            crop_click,
        )
    }

    /// 目标帧和结果帧使用不同文件名，不能覆盖同一事件的原始点击证据。
    pub(crate) fn save_target(
        &self,
        sequence: u64,
        frame: &EvidenceFrame,
        captured_at_ms: u64,
        capture_duration_ms: u64,
        pointer: Option<ScreenPoint>,
    ) -> Result<ScreenshotEvidence, InspectionFailure> {
        self.save_named(
            &format!("{sequence}-target"),
            frame,
            captured_at_ms,
            capture_duration_ms,
            pointer,
            true,
        )
    }

    fn save_named(
        &self,
        name: &str,
        frame: &EvidenceFrame,
        captured_at_ms: u64,
        capture_duration_ms: u64,
        pointer: Option<ScreenPoint>,
        crop_click: bool,
    ) -> Result<ScreenshotEvidence, InspectionFailure> {
        if frame.format() != EvidencePixelFormat::Rgba8 {
            return Err(InspectionFailure::InvalidGeometry);
        }
        let path = format!("evidence/{name}.png");
        write_png(
            &self.directory.join(&path),
            frame.width(),
            frame.height(),
            frame.pixels(),
        )?;
        let crop_result = if crop_click {
            pointer
                .and_then(|point| crop_pixels(frame, point))
                .map(|(bounds, pixels)| {
                    let path = format!("evidence/{name}-crop.png");
                    write_png(
                        &self.directory.join(&path),
                        bounds.width as u32,
                        bounds.height as u32,
                        &pixels,
                    )?;
                    Ok(ScreenshotCrop { path, bounds })
                })
                .transpose()
        } else {
            Ok(None)
        };
        let (crop, crop_failure) = match crop_result {
            Ok(crop) => (crop, None),
            Err(reason) => (None, Some(reason)),
        };
        Ok(ScreenshotEvidence {
            stabilized: false,
            click_color: if crop_click {
                pointer.and_then(|point| crate::click_contrast::color(frame, point))
            } else {
                None
            },
            path,
            captured_at_ms,
            capture_duration_ms,
            screen_bounds: frame.bounds(),
            width: frame.width(),
            height: frame.height(),
            pointer,
            crop,
            crop_failure,
        })
    }
}

/// 使用原始像素裁切，不缩放坐标；负屏幕原点先转换为帧本地坐标。
fn crop_pixels(frame: &EvidenceFrame, point: ScreenPoint) -> Option<(InspectionRect, Vec<u8>)> {
    if !frame.bounds().contains(point) {
        return None;
    }
    let x = point.x - frame.bounds().x as i32;
    let y = point.y - frame.bounds().y as i32;
    // 局部证据最大 192×192，边缘按实际图像范围裁切。
    let (left, top) = ((x - 96).max(0) as u32, (y - 96).max(0) as u32);
    let (right, bottom) = (
        ((x + 96) as u32).min(frame.width()),
        ((y + 96) as u32).min(frame.height()),
    );
    let mut pixels = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
    for row in top..bottom {
        let start = ((row * frame.width() + left) * 4) as usize;
        pixels.extend_from_slice(&frame.pixels()[start..start + ((right - left) * 4) as usize]);
    }
    Some((
        InspectionRect {
            x: left.into(),
            y: top.into(),
            width: (right - left).into(),
            height: (bottom - top).into(),
        },
        pixels,
    ))
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), InspectionFailure> {
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        let pending = path.with_extension("pending");
        let file = BufWriter::new(File::create(&pending)?);
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // 录制优先有界写入延迟；Fastest 仍是无损 PNG，避免排队影响证据完整性。
        encoder.set_compression(png::Compression::Fastest);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
        writer.finish()?;
        std::fs::rename(pending, path)?;
        Ok(())
    };
    write().map_err(|_| InspectionFailure::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_origin_click_crop_uses_frame_local_pixels() {
        let frame = EvidenceFrame::new(
            InspectionRect {
                x: -200.0,
                y: -50.0,
                width: 300.0,
                height: 200.0,
            },
            300,
            200,
            EvidencePixelFormat::Rgba8,
            vec![42; 300 * 200 * 4],
        )
        .unwrap();
        let (bounds, pixels) = crop_pixels(&frame, ScreenPoint { x: -199, y: -49 }).unwrap();
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (0.0, 0.0, 97.0, 97.0)
        );
        assert_eq!(pixels.len(), 97 * 97 * 4);
        assert!(crop_pixels(&frame, ScreenPoint { x: -201, y: 0 }).is_none());
    }
}
