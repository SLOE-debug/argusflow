//! 进程多窗口 Scene 枚举与 Small→Medium 文本解析。

use std::sync::Arc;

use argusflow_core::{AutomationError, BackendKind, RunTraceContext, VisualQuery};

use super::{SceneRefreshPolicy, VisionRuntime};
use crate::{
    AppScene, AppWindowScene, OcrProfile, ResolvedTextTarget, VisionError, WindowInventory,
};

impl VisionRuntime {
    /// 枚举进程当前窗口，并为每个 HWND 独立取得稳定 Scene。
    pub async fn current_app_scene(
        &self,
        inventory: &dyn WindowInventory,
        process_id: u32,
        policy: &SceneRefreshPolicy,
        trace_context: Option<&RunTraceContext>,
    ) -> Result<Arc<AppScene>, VisionError> {
        let windows = inventory.windows_for_process(process_id)?;
        if windows.is_empty() {
            return Err(VisionError::CaptureUnavailable {
                message: format!("process {process_id} has no visible capturable windows"),
            });
        }
        let mut scenes = Vec::with_capacity(windows.len());
        let mut first_capture_error = None;
        for window in windows {
            let scene_result = match trace_context {
                Some(context) => {
                    self.current_scene_for_run(window.identity, policy, context)
                        .await
                }
                None => self.current_scene(window.identity, policy).await,
            };
            match scene_result {
                Ok(scene) => scenes.push(AppWindowScene { window, scene }),
                Err(error) => {
                    first_capture_error.get_or_insert(error);
                }
            }
        }
        if scenes.is_empty() {
            return Err(
                first_capture_error.unwrap_or_else(|| VisionError::CaptureUnavailable {
                    message: format!("process {process_id} has no capturable window scenes"),
                }),
            );
        }
        Ok(Arc::new(AppScene {
            process_id,
            windows: scenes,
        }))
    }

    /// Small 首选、Medium 升级，并在低置信度失败路径使用二值化 Medium。
    pub async fn resolve_text(
        &self,
        inventory: &dyn WindowInventory,
        process_id: u32,
        query: &VisualQuery,
        minimum_confidence: f32,
        trace_context: Option<&RunTraceContext>,
    ) -> Result<ResolvedTextTarget, AutomationError> {
        let small = self
            .current_app_scene(
                inventory,
                process_id,
                &SceneRefreshPolicy::small(),
                trace_context,
            )
            .await
            .map_err(vision_runtime_error)?;
        let small_matches = crate::matching_app_nodes(&small, query);
        if let [candidate] = small_matches.as_slice()
            && candidate.node.confidence >= minimum_confidence
        {
            return Ok(owned_target(*candidate));
        }

        let mut medium_policy = SceneRefreshPolicy::medium();
        medium_policy.force_full_ocr = true;
        let medium = self
            .current_app_scene(inventory, process_id, &medium_policy, trace_context)
            .await
            .map_err(vision_runtime_error)?;
        let candidates = crate::matching_app_nodes(&medium, query);
        match candidates.as_slice() {
            [candidate] if candidate.node.confidence >= minimum_confidence => {
                Ok(owned_target(*candidate))
            }
            candidates if candidates.len() > 1 => Err(AutomationError::AmbiguousTarget {
                query: query.text.clone(),
                matches: candidates.len(),
                details: "Small/Medium escalation still produced multiple window nodes".to_owned(),
            }),
            _ => {
                let mut binary_policy = SceneRefreshPolicy::medium();
                binary_policy.force_full_ocr = true;
                binary_policy.ocr = OcrProfile::medium_binary();
                let binary = self
                    .current_app_scene(inventory, process_id, &binary_policy, trace_context)
                    .await
                    .map_err(vision_runtime_error)?;
                finish_binary_resolution(&binary, query, minimum_confidence)
            }
        }
    }
}

/// 对最后一次二值化观察执行严格 0/1/N 和置信度判定。
fn finish_binary_resolution(
    scene: &Arc<AppScene>,
    query: &VisualQuery,
    minimum_confidence: f32,
) -> Result<ResolvedTextTarget, AutomationError> {
    let candidates = crate::matching_app_nodes(scene, query);
    match candidates.as_slice() {
        [] => Err(AutomationError::TargetNotFound {
            query: query.text.clone(),
            details: "Small, Medium, and binary-enhanced Medium OCR found no matching text"
                .to_owned(),
        }),
        [candidate] if candidate.node.confidence >= minimum_confidence => {
            Ok(owned_target(*candidate))
        }
        [candidate] => Err(AutomationError::TargetNotFound {
            query: query.text.clone(),
            details: format!(
                "binary-enhanced Medium confidence {:.0}% is below the required {:.0}%",
                candidate.node.confidence * 100.0,
                minimum_confidence * 100.0,
            ),
        }),
        candidates => Err(AutomationError::AmbiguousTarget {
            query: query.text.clone(),
            matches: candidates.len(),
            details: "binary-enhanced Medium OCR produced multiple window nodes".to_owned(),
        }),
    }
}

/// 将短期 AppScene 借用转换成可跨输入阶段持有的目标事实。
fn owned_target(candidate: crate::AppNodeRef<'_>) -> ResolvedTextTarget {
    ResolvedTextTarget {
        window: candidate.window.clone(),
        scene: candidate.scene.clone(),
        node: candidate.node.clone(),
    }
}

/// 将运行时故障统一归因到内部 Small→Medium 视觉引擎。
fn vision_runtime_error(error: VisionError) -> AutomationError {
    match error {
        VisionError::CaptureUnavailable { message }
        | VisionError::WorkerUnavailable { message }
        | VisionError::OcrFailed { message }
        | VisionError::Protocol { message } => AutomationError::BackendUnavailable {
            backend: BackendKind::OcrSmall,
            message,
        },
        other => AutomationError::BackendFailed {
            backend: BackendKind::OcrSmall,
            message: other.to_string(),
        },
    }
}
