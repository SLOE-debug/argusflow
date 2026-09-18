//! OCR 图像坐标与采样来源版本同时交付。
use crate::{OcrError, OcrResult};
use argusflow_capture_contracts::{CaptureError, ClockTime, ScreenRect, Version};
use argusflow_core::ScreenPoint;
use std::sync::Arc;

/// 区域采样与模型识别保留各自的错误上下文。
#[derive(Debug, Clone, thiserror::Error)]
pub enum SampledOcrError {
    /// 区域采样失败。
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// 模型识别失败。
    #[error(transparent)]
    Ocr(#[from] OcrError),
}
/// 保留完整区域与来源版本的 OCR 结果。
#[derive(Debug, Clone)]
pub struct SampledOcrResult {
    pub(crate) token: argusflow_capture_contracts::ContentToken,
    pub(crate) result: Arc<OcrResult>,
    pub(crate) version: Version,
    pub(crate) bounds: ScreenRect,
    pub(crate) through: ClockTime,
    pub(crate) reused: bool,
}
impl SampledOcrResult {
    /// 只保留完整落在指定屏幕区域内的文字，再查询可防止把包围矩形中的空隙当作目标。
    /// 返回独立结果，不修改共享缓存；采样票据仍覆盖整个原始区域。
    pub fn restricted_to(&self, regions: &[ScreenRect]) -> Self {
        let mut result = (*self.result).clone();
        result.blocks.retain(|block| {
            regions.iter().any(|region| {
                block.polygon().iter().all(|point| {
                    let x = f64::from(self.bounds.x()) + f64::from(point.x);
                    let y = f64::from(self.bounds.y()) + f64::from(point.y);
                    x >= f64::from(region.x())
                        && y >= f64::from(region.y())
                        && x <= f64::from(region.x()) + f64::from(region.width())
                        && y <= f64::from(region.y()) + f64::from(region.height())
                })
            })
        });
        Self {
            result: Arc::new(result),
            ..self.clone()
        }
    }
    /// 原图坐标中的完整 OCR 结果。
    pub fn result(&self) -> &OcrResult {
        &self.result
    }
    /// 本次健康采样确认的版本；缓存复用也更新此字段。
    pub fn version(&self) -> Version {
        self.version
    }
    /// OCR 输入区域在虚拟桌面的范围。
    pub fn bounds(&self) -> ScreenRect {
        self.bounds
    }
    /// 采样健康处理水位。
    pub fn observed_through(&self) -> ClockTime {
        self.through
    }
    /// 是否复用了像素内容相同的 OCR 结果。
    pub fn reused(&self) -> bool {
        self.reused
    }
    /// 指定文本块的屏幕物理像素四边形，子像素角点四舍五入到整数。
    pub fn screen_polygon(&self, index: usize) -> Option<[ScreenPoint; 4]> {
        let block = self.result.blocks().get(index)?;
        Some(block.polygon().map(|point| ScreenPoint {
            x: (f64::from(self.bounds.x()) + f64::from(point.x)).round() as i32,
            y: (f64::from(self.bounds.y()) + f64::from(point.y)).round() as i32,
        }))
    }
}
