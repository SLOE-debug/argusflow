//! 先完整检测文字几何，再以公共差分判断哪些文字块需要重新识别。
use super::cache::RecognitionCache;
use crate::{OcrError, OcrResult, TextBlock};
use argusflow_core::{ImagePoint, Operation};
use image::RgbImage;

pub(super) fn process(
    image: RgbImage,
    cache: &mut RecognitionCache,
    operation: &Operation,
    detect: impl FnOnce(&RgbImage) -> Result<Vec<[ImagePoint; 4]>, OcrError>,
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
    let polygons = detect(&image)?;
    let mut blocks = Vec::with_capacity(polygons.len());
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
#[path = "../../../../tests/argusflow-vision/unit/ocr/incremental.rs"]
mod tests;
