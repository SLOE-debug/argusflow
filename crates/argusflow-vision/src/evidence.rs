//! 视觉 backend 的 PreparedDiagnostics 实现。

use std::sync::Arc;

use argusflow_agent::{
    EvidenceArtifact, EvidenceArtifactData, EvidenceArtifactKind, EvidenceBundle,
    EvidenceCaptureError, EvidenceCaptureRequest, PreparedDiagnostics,
};
use argusflow_core::{BackendKind, WindowIdentity};
use argusflow_query::BranchPath;
use async_trait::async_trait;
use serde_json::json;

use crate::{runtime::VisionRuntime, scene::VisualScene};

/// 已绑定窗口、查询和共享 runtime 的视觉 evidence collector。
#[derive(Debug)]
pub struct VisionPreparedDiagnostics {
    /// 共享视觉 runtime。
    runtime: Arc<VisionRuntime>,
    /// prepare 阶段冻结的窗口身份。
    window: WindowIdentity,
    /// prepare 阶段冻结的查询文本。
    query: String,
    /// 产生该 candidate 的 backend。
    backend: BackendKind,
}

impl VisionPreparedDiagnostics {
    /// 创建视觉诊断对象。
    pub fn new(
        runtime: Arc<VisionRuntime>,
        window: WindowIdentity,
        query: impl Into<String>,
        backend: BackendKind,
    ) -> Self {
        Self {
            runtime,
            window,
            query: query.into(),
            backend,
        }
    }
}

#[async_trait]
impl PreparedDiagnostics for VisionPreparedDiagnostics {
    fn backend(&self) -> BackendKind {
        self.backend
    }

    async fn capture(
        &self,
        request: EvidenceCaptureRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        if request.explain.backend != self.backend {
            return Err(EvidenceCaptureError::BackendMismatch);
        }
        let branch_path = request
            .explain
            .branch_path
            .clone()
            .unwrap_or_else(BranchPath::root);
        let mut bundle = EvidenceBundle::new(
            self.backend,
            branch_path,
            request.trigger,
            self.query.clone(),
        );
        let health = self.runtime.health();
        bundle.push(EvidenceArtifact {
            kind: EvidenceArtifactKind::ExecutionContext,
            relative_path: "execution_context.json".into(),
            sensitive: false,
            data: EvidenceArtifactData::Json(json!({
                "window": self.window,
                "capture_ready": health.capture_ready,
                "worker_ready": health.worker_ready,
                "protocol_version": health.worker.protocol_version,
                "capture_fps": self.runtime.metrics().capture_fps(),
            })),
        });
        bundle.push(EvidenceArtifact {
            kind: EvidenceArtifactKind::PlannerExplain,
            relative_path: "planner_explain.json".into(),
            sensitive: false,
            data: EvidenceArtifactData::Json(serde_json::to_value(&request.explain).map_err(
                |error| EvidenceCaptureError::CaptureFailed {
                    message: format!("failed to encode planner explain: {error}"),
                },
            )?),
        });
        bundle.push(EvidenceArtifact {
            kind: EvidenceArtifactKind::Logs,
            relative_path: "vision_metrics.json".into(),
            sensitive: false,
            data: EvidenceArtifactData::Json(
                serde_json::to_value(self.runtime.metrics().snapshot()).map_err(|error| {
                    EvidenceCaptureError::CaptureFailed {
                        message: format!("failed to encode vision metrics: {error}"),
                    }
                })?,
            ),
        });
        let Some(scene) = self.runtime.cached_scene() else {
            return Ok(bundle);
        };
        if scene.window != self.window {
            return Ok(bundle);
        }
        let scene_artifact = scene_artifact(&scene, request.retention.persist_text_values);
        bundle.push(scene_artifact);
        if request.retention.persist_text_values {
            bundle.push(EvidenceArtifact {
                kind: EvidenceArtifactKind::Logs,
                relative_path: "compact_text.txt".into(),
                sensitive: true,
                data: EvidenceArtifactData::Text(scene.compact_text.clone()),
            });
            bundle.push(EvidenceArtifact {
                kind: EvidenceArtifactKind::Logs,
                relative_path: "spatial_text.txt".into(),
                sensitive: true,
                data: EvidenceArtifactData::Text(scene.spatial_text.clone()),
            });
        }
        Ok(bundle)
    }
}

/// 按 retention policy 生成包含或不包含文本的 scene evidence。
fn scene_artifact(scene: &VisualScene, persist_text: bool) -> EvidenceArtifact {
    let data = if persist_text {
        serde_json::to_value(scene).unwrap_or_else(|_| json!({"scene_id": scene.scene_id.get()}))
    } else {
        json!({
            "scene_id": scene.scene_id.get(),
            "frame_id": scene.frame_id.get(),
            "topology_generation": scene.topology_generation.get(),
            "ocr": scene.ocr,
            "node_count": scene.nodes.len(),
            "regions": scene.regions,
            "nodes": scene.nodes.iter().map(|node| json!({
                "id": node.id.get(),
                "bbox": node.bbox,
                "confidence": node.confidence,
                "source": node.source,
            })).collect::<Vec<_>>(),
        })
    };
    EvidenceArtifact {
        kind: EvidenceArtifactKind::OcrRegions,
        relative_path: "ocr_regions.json".into(),
        sensitive: persist_text,
        data: EvidenceArtifactData::Json(data),
    }
}
