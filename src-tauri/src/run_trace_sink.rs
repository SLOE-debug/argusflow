//! Vision Runtime 诊断事实到本地 Run Artifact Store 的宿主适配。

use std::sync::Arc;

use argusflow_core::RunTraceContext;
use argusflow_runtime::{
    FileRunTraceStore, RunPixelPoint, RunPixelRect, RunSceneNodeProjection, RunSceneNodeRef,
    RunSceneProjection, RunSceneWindowProjection, RunVisualQueryMetrics, RunVisualQueryTrace,
    RunVisualSelectionOutcome,
};
use argusflow_vision::{
    AppScene, CapturedFrame, OcrRequest, OcrResponse, SceneNodeIdentity, SceneProjection,
    VisionQueryMetrics, VisionSelectionOutcome, VisionTraceSink, VisualNodeSource,
    encode_bgra_as_bmp, project_app_scene,
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
            "schema_version": 2,
            "run_id": context.run_id,
            "node_id": context.node_id,
            "node_sequence": context.node_sequence,
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
            request.window.handle,
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
        candidates: &[SceneNodeIdentity],
        selected: Option<&SceneNodeIdentity>,
        outcome: VisionSelectionOutcome,
        metrics: VisionQueryMetrics,
    ) {
        let trace = RunVisualQueryTrace {
            schema_version: 2,
            run_id: context.run_id,
            node_id: context.node_id.clone(),
            node_sequence: context.node_sequence,
            query: query_source.to_owned(),
            outcome: map_outcome(outcome),
            candidate_nodes: candidates.iter().map(map_node_ref).collect(),
            selected_node: selected.map(map_node_ref),
            metrics: map_metrics(metrics),
            projection: map_projection(project_app_scene(scene)),
        };
        let _ = self.store.persist_query_trace(
            context.run_id,
            &context.node_id,
            context.node_sequence,
            &trace,
        );
    }
}

fn map_node_ref(identity: &SceneNodeIdentity) -> RunSceneNodeRef {
    RunSceneNodeRef {
        window_handle: identity.window_handle.to_string(),
        scene_id: identity.scene_id,
        node_id: identity.node_id.clone(),
    }
}

fn map_outcome(outcome: VisionSelectionOutcome) -> RunVisualSelectionOutcome {
    match outcome {
        VisionSelectionOutcome::NotFound => RunVisualSelectionOutcome::NotFound,
        VisionSelectionOutcome::Unique => RunVisualSelectionOutcome::Unique,
        VisionSelectionOutcome::Multiple => RunVisualSelectionOutcome::Multiple,
        VisionSelectionOutcome::Ambiguous => RunVisualSelectionOutcome::Ambiguous,
        VisionSelectionOutcome::RejectedConfidence => RunVisualSelectionOutcome::RejectedConfidence,
    }
}

fn map_metrics(metrics: VisionQueryMetrics) -> RunVisualQueryMetrics {
    RunVisualQueryMetrics {
        elapsed_us: metrics.elapsed_us,
        exact_index_hits: metrics.exact_index_hits,
        scanned_nodes: metrics.scanned_nodes,
        spatial_candidates: metrics.spatial_candidates,
    }
}

fn map_projection(projection: SceneProjection) -> RunSceneProjection {
    RunSceneProjection {
        schema_version: projection.schema_version,
        windows: projection
            .windows
            .into_iter()
            .map(|window| RunSceneWindowProjection {
                window_handle: window.window_handle.to_string(),
                scene_id: window.scene_id,
                frame_id: window.frame_id,
                z_order: window.z_order,
                foreground: window.foreground,
                screen_bounds: map_rect(window.screen_bounds),
                frame_bounds: map_rect(window.frame_bounds),
            })
            .collect(),
        nodes: projection
            .nodes
            .into_iter()
            .map(|node| RunSceneNodeProjection {
                node_id: node.node_id,
                scene_id: node.scene_id,
                frame_id: node.frame_id,
                window_handle: node.window_handle.to_string(),
                text: node.text,
                frame_bbox: map_rect(node.frame_bbox),
                screen_bbox: map_rect(node.screen_bbox),
                polygon: node
                    .polygon
                    .into_iter()
                    .map(|point| RunPixelPoint {
                        x: point.x,
                        y: point.y,
                    })
                    .collect(),
                confidence: node.confidence,
                source: match node.source {
                    VisualNodeSource::OcrSmall => "ocr_small",
                    VisualNodeSource::OcrMedium => "ocr_medium",
                }
                .to_owned(),
            })
            .collect(),
    }
}

fn map_rect(rect: argusflow_vision::PhysicalRect) -> RunPixelRect {
    RunPixelRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}
