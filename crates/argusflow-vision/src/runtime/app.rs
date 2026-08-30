//! 进程多窗口 Scene 枚举与按置信度升级的文本解析。

use std::sync::Arc;

use argusflow_core::{AutomationError, BackendKind, RunTraceContext, VisualQuery};

use super::{SceneRefreshPolicy, VisionRuntime};
use crate::{
    AppScene, AppWindowScene, OcrProfile, ResolvedTextTarget, VisionError, VisionQueryPlan,
    WindowInventory, evaluate_vision_query, require_unique,
};

#[cfg(test)]
mod tests;

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
        for window in windows {
            let scene_result = match trace_context {
                Some(context) => {
                    self.current_scene_for_run(window.identity, policy, context)
                        .await
                }
                None => self.current_scene(window.identity, policy).await,
            };
            let scene = scene_result.map_err(|error| VisionError::CaptureUnavailable {
                message: format!(
                    "process {process_id} scene is incomplete because window {} failed: {error}",
                    window.identity.handle,
                ),
            })?;
            scenes.push(AppWindowScene { window, scene });
        }
        Ok(Arc::new(AppScene {
            process_id,
            windows: scenes,
        }))
    }

    /// Small 首选；0/N 结果立即交给外层等待器，只有唯一低置信度命中才升级模型。
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
        match small_matches.as_slice() {
            [] => {
                return Err(AutomationError::TargetNotFound {
                    query: query.text.clone(),
                    details: "Small OCR complete scene contains no matching text node".to_owned(),
                });
            }
            [candidate] if candidate.node.confidence >= minimum_confidence => {
                return Ok(owned_target(*candidate));
            }
            [_candidate] => {
                // 唯一候选已证明文字语义可能存在，才值得支付更高精度模型的成本。
            }
            candidates => {
                return Err(AutomationError::AmbiguousTarget {
                    query: query.text.clone(),
                    matches: candidates.len(),
                    details: "Small OCR produced multiple matching window nodes".to_owned(),
                });
            }
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

    /// 使用一次性编译的 AQL 计划执行严格唯一查询；仅为低置信度唯一项升级模型。
    pub async fn resolve_query(
        &self,
        inventory: &dyn WindowInventory,
        process_id: u32,
        plan: &VisionQueryPlan,
        query_source: &str,
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
            .map_err(|error| vision_query_runtime_error(error, query_source))?;
        let small_result = evaluate_vision_query(&small, plan, query_source)?;
        match require_unique(&small_result, query_source) {
            Ok((candidate, metrics)) if candidate.node.confidence >= minimum_confidence => {
                self.record_query_trace(
                    trace_context,
                    &small,
                    query_source,
                    &small_result.matches,
                    Some(candidate.node.id),
                    crate::VisionSelectionOutcome::Unique,
                    metrics,
                );
                return Ok(owned_target(candidate));
            }
            Ok((_candidate, _metrics)) => {
                // 只有唯一但低置信度的候选才升级；纯 miss 必须先让等待器取得更新帧。
            }
            Err(error @ AutomationError::TargetNotFound { .. }) => {
                self.record_query_trace(
                    trace_context,
                    &small,
                    query_source,
                    &small_result.matches,
                    None,
                    crate::VisionSelectionOutcome::NotFound,
                    small_result.metrics,
                );
                return Err(error);
            }
            Err(error @ AutomationError::AmbiguousTarget { .. }) => {
                self.record_query_trace(
                    trace_context,
                    &small,
                    query_source,
                    &small_result.matches,
                    None,
                    crate::VisionSelectionOutcome::Ambiguous,
                    small_result.metrics,
                );
                return Err(error);
            }
            Err(error) => return Err(error),
        }

        let mut medium_policy = SceneRefreshPolicy::medium();
        medium_policy.force_full_ocr = true;
        let medium = self
            .current_app_scene(inventory, process_id, &medium_policy, trace_context)
            .await
            .map_err(|error| vision_query_runtime_error(error, query_source))?;
        let medium_result = evaluate_vision_query(&medium, plan, query_source)?;
        match require_unique(&medium_result, query_source) {
            Ok((candidate, metrics)) if candidate.node.confidence >= minimum_confidence => {
                self.record_query_trace(
                    trace_context,
                    &medium,
                    query_source,
                    &medium_result.matches,
                    Some(candidate.node.id),
                    crate::VisionSelectionOutcome::Unique,
                    metrics,
                );
                return Ok(owned_target(candidate));
            }
            Err(error @ AutomationError::AmbiguousTarget { .. }) => {
                self.record_query_trace(
                    trace_context,
                    &medium,
                    query_source,
                    &medium_result.matches,
                    None,
                    crate::VisionSelectionOutcome::Ambiguous,
                    medium_result.metrics,
                );
                return Err(error);
            }
            _ => {}
        }

        let mut binary_policy = SceneRefreshPolicy::medium();
        binary_policy.force_full_ocr = true;
        binary_policy.ocr = OcrProfile::medium_binary();
        let binary = self
            .current_app_scene(inventory, process_id, &binary_policy, trace_context)
            .await
            .map_err(|error| vision_query_runtime_error(error, query_source))?;
        let binary_result = evaluate_vision_query(&binary, plan, query_source)?;
        let unique = require_unique(&binary_result, query_source);
        let (candidate, metrics) = match unique {
            Ok(unique) => unique,
            Err(error) => {
                self.record_query_trace(
                    trace_context,
                    &binary,
                    query_source,
                    &binary_result.matches,
                    None,
                    if binary_result.matches.is_empty() {
                        crate::VisionSelectionOutcome::NotFound
                    } else {
                        crate::VisionSelectionOutcome::Ambiguous
                    },
                    binary_result.metrics,
                );
                return Err(error);
            }
        };
        if candidate.node.confidence < minimum_confidence {
            self.record_query_trace(
                trace_context,
                &binary,
                query_source,
                &binary_result.matches,
                None,
                crate::VisionSelectionOutcome::RejectedConfidence,
                metrics,
            );
            return Err(AutomationError::TargetNotFound {
                query: query_source.to_owned(),
                details: format!(
                    "binary-enhanced Medium confidence {:.0}% is below the required {:.0}%",
                    candidate.node.confidence * 100.0,
                    minimum_confidence * 100.0,
                ),
            });
        }
        self.record_query_trace(
            trace_context,
            &binary,
            query_source,
            &binary_result.matches,
            Some(candidate.node.id),
            crate::VisionSelectionOutcome::Unique,
            metrics,
        );
        Ok(owned_target(candidate))
    }

    /// 只有宿主提供 Run Trace 上下文时才构建并写入 Scene 投影。
    pub(crate) fn record_query_trace(
        &self,
        context: Option<&RunTraceContext>,
        scene: &AppScene,
        query_source: &str,
        candidates: &[crate::AppNodeRef<'_>],
        selected_id: Option<crate::VisualNodeId>,
        outcome: crate::VisionSelectionOutcome,
        metrics: crate::VisionQueryMetrics,
    ) {
        let (Some(context), Some(sink)) = (context, self.trace_sink.as_ref()) else {
            return;
        };
        let candidate_ids = candidates
            .iter()
            .map(|candidate| candidate.node.id)
            .collect::<Vec<_>>();
        sink.record_query(
            context,
            scene,
            query_source,
            &candidate_ids,
            selected_id,
            outcome,
            metrics,
        );
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
        VisionError::CaptureUnavailable { message } => AutomationError::ObservationIncomplete {
            query: "OCR scene".to_owned(),
            details: format!("; {message}"),
        },
        VisionError::WorkerUnavailable { message }
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

/// 为 AQL Scene 完整性错误补充真实查询源码，避免等待与追踪丢失上下文。
fn vision_query_runtime_error(error: VisionError, query_source: &str) -> AutomationError {
    match vision_runtime_error(error) {
        AutomationError::ObservationIncomplete { details, .. } => {
            AutomationError::ObservationIncomplete {
                query: query_source.to_owned(),
                details,
            }
        }
        other => other,
    }
}
