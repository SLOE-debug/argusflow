//! 仅缓存当前引擎上一张成功图像；区域复用不改变文字检测与裁剪语义。
use crate::{OcrError, OcrResult, TextBlock};
use argusflow_core::{ImagePoint, Operation};
use argusflow_image::{ChangeRegion, DifferencePolicy, ImageView, changed_regions};
use image::RgbImage;

/// 引擎线程独占缓存，模型和预处理配置在该线程生命周期中不变。
#[derive(Default)]
pub(crate) struct RecognitionCache {
    previous: Option<(RgbImage, OcrResult)>,
}
impl RecognitionCache {
    pub fn changes(
        &self,
        image: &RgbImage,
        operation: &Operation,
    ) -> Result<Option<Vec<ChangeRegion>>, OcrError> {
        let Some((before, _)) = &self.previous else {
            return Ok(None);
        };
        if before.dimensions() != image.dimensions() {
            return Ok(None);
        }
        changed_regions(
            ImageView::triples(before.width(), before.height(), before.as_raw())
                .map_err(capture_error)?,
            ImageView::triples(image.width(), image.height(), image.as_raw())
                .map_err(capture_error)?,
            DifferencePolicy::default(),
            operation,
        )
        .map(Some)
        .map_err(capture_error)
    }
    pub fn result(&self) -> Option<&OcrResult> {
        self.previous.as_ref().map(|(_, result)| result)
    }
    pub fn block(&self, polygon: &[ImagePoint; 4], changes: &[ChangeRegion]) -> Option<TextBlock> {
        // 双线性插值可能读取边界相邻像素；外扩两个像素后才允许复用。
        let left = polygon
            .iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            - 2.0;
        let top = polygon
            .iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            - 2.0;
        let right = polygon
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            + 2.0;
        let bottom = polygon
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            + 2.0;
        if changes.iter().any(|change| {
            let rect = change.bounds;
            (rect.x() as f32) < right
                && (rect.y() as f32) < bottom
                && rect.right() as f32 > left
                && rect.bottom() as f32 > top
        }) {
            return None;
        }
        self.result()?
            .blocks()
            .iter()
            .find(|block| block.polygon() == polygon)
            .cloned()
    }
    pub fn commit(&mut self, image: RgbImage, result: OcrResult) {
        // 缓存是额外常驻内存，限定 64 MiB；大图仍正常识别但不保留副本。
        self.previous = (image.as_raw().len() <= 64 * 1024 * 1024).then_some((image, result));
    }
}
fn capture_error(error: argusflow_capture_contracts::CaptureError) -> OcrError {
    argusflow_core::Failure::new(error.kind(), "ocr_difference", error.to_string()).into()
}
