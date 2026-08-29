//! Vision Runtime 到宿主 Run Artifact Store 的可选诊断边界。

use crate::{CapturedFrame, OcrRequest, OcrResponse};
use argusflow_core::RunTraceContext;

/// 宿主接收捕获帧、OCR ROI 与 exact model input 的 best-effort 接口。
pub trait VisionTraceSink: std::fmt::Debug + Send + Sync + 'static {
    /// 一次 OCR 响应完成后保存同源三层图像和结构化结果。
    fn record_ocr(
        &self,
        context: &RunTraceContext,
        frame: &CapturedFrame,
        request: &OcrRequest,
        response: &OcrResponse,
    );
}
