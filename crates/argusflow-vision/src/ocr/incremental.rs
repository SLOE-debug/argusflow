//! 精确差分、行带检测和文字块复用；仅完整成功后提交下一帧缓存。
use super::cache::RecognitionCache;
use crate::{OcrError, OcrResult, TextBlock};
use argusflow_core::{ImagePoint, Operation};
use image::RgbImage;

pub(super) fn process(
    image: RgbImage,
    cache: &mut RecognitionCache,
    operation: &Operation,
    mut detect: impl FnMut(&RgbImage) -> Result<Vec<[ImagePoint; 4]>, OcrError>,
    mut recognize: impl FnMut(&RgbImage, [ImagePoint; 4]) -> Result<Option<TextBlock>, OcrError>,
) -> Result<OcrResult, OcrError> {
    operation.check("ocr_difference")?;
    let changes = cache.changes(&image, operation)?;
    if changes.as_ref().is_some_and(Vec::is_empty)
        && let Some(result) = cache.result()
    {
        operation.check("ocr_cached_complete")?;
        return Ok(result.clone());
    }
    let bands = match (&changes, cache.result()) {
        (Some(changes), Some(previous)) => super::regions::plan(changes, previous, image.height()),
        _ => vec![super::regions::Band {
            top: 0,
            bottom: image.height(),
        }],
    };
    let mut blocks: Vec<_> = cache
        .result()
        .filter(|_| changes.is_some())
        .into_iter()
        .flat_map(|r| r.blocks())
        .filter(|block| !bands.iter().any(|band| band.intersects(block)))
        .cloned()
        .collect();
    let mut polygons = Vec::new();
    for band in &bands {
        operation.check("ocr_detection_band")?;
        if band.top == 0 && band.bottom == image.height() {
            polygons = detect(&image)?;
            break;
        }
        let cropped =
            image::imageops::crop_imm(&image, 0, band.top, image.width(), band.bottom - band.top)
                .to_image();
        let detected = detect(&cropped)?;
        // 新文字可能比旧文字更高；贴近裁剪边缘时必须重新完整检测，不能提交截断文字。
        if detected.iter().flatten().any(|p| {
            (band.top > 0 && p.y <= 2.0)
                || (band.bottom < image.height() && p.y >= cropped.height() as f32 - 2.0)
        }) {
            blocks.clear();
            polygons = detect(&image)?;
            break;
        }
        polygons.extend(detected.into_iter().map(|polygon| {
            polygon.map(|p| ImagePoint {
                x: p.x,
                y: p.y + band.top as f32,
            })
        }));
    }
    for polygon in polygons {
        operation.check("ocr_region")?;
        let cached = changes
            .as_ref()
            .and_then(|changes| cache.block(&polygon, changes));
        if let Some(block) = cached {
            blocks.push(block);
        } else if let Some(block) = recognize(&image, polygon)? {
            blocks.push(block);
        }
    }
    blocks.sort_by(|a, b| {
        a.polygon()[0]
            .y
            .total_cmp(&b.polygon()[0].y)
            .then_with(|| a.polygon()[0].x.total_cmp(&b.polygon()[0].x))
    });
    let result = OcrResult {
        blocks,
        width: image.width(),
        height: image.height(),
    };
    operation.check("ocr_complete")?;
    cache.commit(image, result.clone());
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/ocr/bands.rs"]
mod band_tests;
#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/ocr/incremental.rs"]
mod tests;
