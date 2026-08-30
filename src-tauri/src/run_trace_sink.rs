//! Vision Runtime 诊断事实到本地 Run Artifact Store 的宿主适配。

use std::sync::Arc;

use argusflow_core::RunTraceContext;
use argusflow_runtime::FileRunTraceStore;
use argusflow_vision::{
    AppScene, CapturedFrame, OcrRequest, OcrResponse, VisionQueryMetrics, VisionSelectionOutcome,
    VisionTraceSink, VisualNodeId, encode_bgra_as_bmp, project_app_scene,
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

    fn record_query(
        &self,
        context: &RunTraceContext,
        scene: &AppScene,
        query_source: &str,
        candidate_ids: &[VisualNodeId],
        selected_id: Option<VisualNodeId>,
        outcome: VisionSelectionOutcome,
        metrics: VisionQueryMetrics,
    ) {
        let projection = project_app_scene(scene);
        let scene_id = scene
            .windows
            .iter()
            .map(|window| window.scene.scene_id.get())
            .max()
            .unwrap_or(0);
        let frame_id = scene
            .windows
            .iter()
            .map(|window| window.scene.frame_id.get())
            .max()
            .unwrap_or(0);
        let outcome = match outcome {
            VisionSelectionOutcome::NotFound => "not_found",
            VisionSelectionOutcome::Unique => "unique",
            VisionSelectionOutcome::Multiple => "multiple",
            VisionSelectionOutcome::Ambiguous => "ambiguous",
            VisionSelectionOutcome::RejectedConfidence => "rejected_confidence",
        };
        let candidate_node_ids = candidate_ids
            .iter()
            .map(|node_id| node_id.get().to_string())
            .collect::<Vec<_>>();
        let selected_node_id = selected_id.map(|node_id| node_id.get().to_string());
        let trace = json!({
            "run_id": context.run_id,
            "node_id": context.node_id,
            "scene_id": scene_id,
            "frame_id": frame_id,
            "query": query_source,
            "outcome": outcome,
            "candidate_node_ids": candidate_node_ids,
            "selected_node_id": selected_node_id,
            "metrics": metrics,
            "projection": projection,
        });
        let _ = self
            .store
            .persist_query_trace(context.run_id, &context.node_id, scene_id, &trace);
    }
}
