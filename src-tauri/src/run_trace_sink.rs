//! Vision Runtime 诊断事实到本地 Run Artifact Store 的宿主适配。

use std::sync::Arc;

use argusflow_core::RunTraceContext;
use argusflow_runtime::FileRunTraceStore;
use argusflow_vision::{
    CapturedFrame, OcrRequest, OcrResponse, VisionTraceSink, encode_bgra_as_bmp,
};
use serde_json::json;

/// 将 Vision 的短期像素转换为 Run-scoped artifact；任何写入失败都不影响 OCR 结果。
#[derive(Debug)]
pub struct RunVisionTraceSink {
    store: Arc<FileRunTraceStore>,
}

impl RunVisionTraceSink {
    /// 绑定与 WorkflowEngine 共用的 Run Store。
    pub fn new(store: Arc<FileRunTraceStore>) -> Self {
        Self { store }
    }
}

impl VisionTraceSink for RunVisionTraceSink {
    fn record_ocr(
        &self,
        context: &RunTraceContext,
        frame: &CapturedFrame,
        request: &OcrRequest,
        response: &OcrResponse,
    ) {
        let Ok(frame_image) = frame.crop(frame.bounds()) else {
            return;
        };
        let Ok(frame_bmp) = encode_bgra_as_bmp(&frame_image) else {
            return;
        };
        let Ok(source_roi_bmp) = encode_bgra_as_bmp(&request.image) else {
            return;
        };
        let request_metadata = json!({
            "schema_version": 1,
            "run_id": context.run_id,
            "node_id": context.node_id,
            "request_id": request.request_id.as_uuid(),
            "frame_id": request.frame_id.get(),
            "topology_generation": request.topology_generation.get(),
            "window": request.window,
            "roi": request.roi,
            "profile": request.profile,
            "source_width": request.image.width,
            "source_height": request.image.height,
            "frame_width": frame.width,
            "frame_height": frame.height,
            "dpi_x": frame.dpi_x,
            "dpi_y": frame.dpi_y,
        });
        let Ok(response_metadata) = serde_json::to_value(response) else {
            return;
        };
        let model_input = response
            .model_input
            .as_ref()
            .map(|artifact| artifact.bytes.as_slice());
        let _ = self.store.persist_ocr_artifacts(
            context.run_id,
            request.frame_id.get(),
            request.request_id.as_uuid(),
            &frame_bmp,
            &source_roi_bmp,
            model_input,
            &request_metadata,
            &response_metadata,
        );
    }
}
