use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use argusflow_agent::{
    MaterializedTarget, PreparedTargetMaterializer, VisualBaseline, VisualMaterializationPlan,
    VisualMaterializationStage, VisualVerificationProvider, VisualVerificationResult,
    WindowContext,
};
use argusflow_core::{AutomationError, BackendKind, VisualQuery, WindowIdentity};
use argusflow_vision::{
    SceneRefreshPolicy, VerificationOutcome, VisionError, VisionRuntime, VisualCondition,
    VisualMatch, evaluate_visual_condition, evaluate_visual_query,
};
use async_trait::async_trait;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::GetWindowRect,
};

use super::surface_transform::SurfaceTransform;

/// 基于共享 VisionRuntime 的 Windows 视觉目标物化器。
#[derive(Debug, Clone)]
pub struct WindowsVisualTargetMaterializer {
    /// 与视觉读取后端共享 capture、OCR worker 和 scene cache。
    runtime: Arc<VisionRuntime>,
    /// 发送前保存的 scene；opaque token 防止 Agent 层依赖视觉实现细节。
    baselines: Arc<Mutex<HashMap<uuid::Uuid, Arc<argusflow_vision::VisualScene>>>>,
}

impl WindowsVisualTargetMaterializer {
    /// 创建绑定共享视觉运行时的物化器。
    pub fn new(runtime: Arc<VisionRuntime>) -> Self {
        Self {
            runtime,
            baselines: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PreparedTargetMaterializer for WindowsVisualTargetMaterializer {
    async fn materialize(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
        plan: &VisualMaterializationPlan,
    ) -> Result<MaterializedTarget, AutomationError> {
        let identity = WindowIdentity {
            handle: window.handle,
            process_id: window.process_id,
        };
        let mut last_error = None;
        for stage in &plan.stages {
            let result = match stage {
                VisualMaterializationStage::Cache => {
                    let mut refresh = SceneRefreshPolicy::tiny();
                    refresh.normalized_query_region = query.region;
                    match self.runtime.lookup_cache(identity, &refresh) {
                        argusflow_vision::CacheLookup::Hit(scene) => Ok(scene),
                        argusflow_vision::CacheLookup::Miss(reason) => {
                            last_error = Some(AutomationError::BackendUnavailable {
                                backend: BackendKind::VisualCache,
                                message: format!("visual cache miss: {reason:?}"),
                            });
                            continue;
                        }
                    }
                }
                VisualMaterializationStage::OcrTiny => {
                    let mut refresh = SceneRefreshPolicy::tiny();
                    refresh.normalized_query_region = query.region;
                    self.runtime
                        .current_scene(identity, &refresh)
                        .await
                        .map_err(|error| map_vision_error(BackendKind::OcrTiny, error))
                }
                VisualMaterializationStage::OcrMedium => {
                    let mut refresh = SceneRefreshPolicy::medium();
                    refresh.normalized_query_region = query.region;
                    self.runtime
                        .current_scene(identity, &refresh)
                        .await
                        .map_err(|error| map_vision_error(BackendKind::OcrMedium, error))
                }
                VisualMaterializationStage::GuiGrounding => {
                    Err(AutomationError::BackendUnavailable {
                        backend: BackendKind::GuiGrounding,
                        message: "GUI grounding materializer is not configured".to_owned(),
                    })
                }
            };
            match result {
                Ok(scene) => {
                    if scene.window != identity {
                        return Err(AutomationError::BackendFailed {
                            backend: BackendKind::VisualCache,
                            message: "visual scene belongs to a different window identity"
                                .to_owned(),
                        });
                    }
                    let node = match evaluate_visual_query(&scene, query) {
                        Ok(VisualMatch::Unique(node)) => node,
                        Err(error @ AutomationError::TargetNotFound { .. }) => {
                            last_error = Some(error);
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    let transform = SurfaceTransform::new(window_bounds(window)?, scene.viewport)?;
                    let mapped = transform.map_rect(node.bbox)?;
                    return Ok(MaterializedTarget {
                        window: window.clone(),
                        scene_id: scene.scene_id.get(),
                        frame_id: scene.frame_id.get(),
                        topology_generation: scene.topology_generation.get(),
                        bounds: mapped.bounds,
                        confidence: node.confidence,
                        safe_point: mapped.safe_point,
                        source_backend: stage_backend(*stage),
                    });
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(
            last_error.unwrap_or_else(|| AutomationError::BackendUnavailable {
                backend: BackendKind::SendInput,
                message: "visual materialization plan has no usable stage".to_owned(),
            }),
        )
    }
}

#[async_trait]
impl VisualVerificationProvider for WindowsVisualTargetMaterializer {
    async fn capture_baseline(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
    ) -> Result<VisualBaseline, AutomationError> {
        let identity = WindowIdentity {
            handle: window.handle,
            process_id: window.process_id,
        };
        let mut refresh = SceneRefreshPolicy::tiny();
        refresh.normalized_query_region = query.region;
        let scene = self
            .runtime
            .current_scene(identity, &refresh)
            .await
            .map_err(|error| map_vision_error(BackendKind::VisualCache, error))?;
        if scene.window != identity {
            return Err(AutomationError::BackendFailed {
                backend: BackendKind::VisualCache,
                message: "visual baseline belongs to a different window identity".to_owned(),
            });
        }
        let baseline = VisualBaseline::new(window.clone());
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(baseline.token(), scene);
        Ok(baseline)
    }

    /// 丢弃动作未开始或执行失败时遗留的 baseline，避免短期 scene 索引持续增长。
    async fn discard_baseline(&self, baseline: VisualBaseline) {
        self.baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&baseline.token());
    }

    async fn verify_new_text(
        &self,
        baseline: VisualBaseline,
        query: &VisualQuery,
    ) -> Result<VisualVerificationResult, AutomationError> {
        let previous = self
            .baselines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&baseline.token())
            .ok_or_else(|| AutomationError::OutcomeUnknown {
                backend: BackendKind::SendInput,
                message: "visual baseline was not found or was already consumed".to_owned(),
            })?;
        let identity = WindowIdentity {
            handle: baseline.window().handle,
            process_id: baseline.window().process_id,
        };
        let mut refresh = SceneRefreshPolicy::medium();
        refresh.normalized_query_region = query.region;
        let current = self
            .runtime
            .current_scene(identity, &refresh)
            .await
            .map_err(|error| map_vision_error(BackendKind::OcrMedium, error))?;
        let condition = VisualCondition::NewTextExistsSince {
            query: query.clone(),
            since_scene_id: previous.scene_id,
            region: None,
        };
        Ok(
            match evaluate_visual_condition(Some(&current), Some(&previous), &condition) {
                VerificationOutcome::Confirmed { .. } => VisualVerificationResult::Confirmed,
                VerificationOutcome::Rejected { reason } => {
                    VisualVerificationResult::Rejected { reason }
                }
                VerificationOutcome::Uncertain { reason } => {
                    VisualVerificationResult::Uncertain { reason }
                }
            },
        )
    }
}

/// 将物化阶段转换为事实来源，供输入证据和错误定位使用。
const fn stage_backend(stage: VisualMaterializationStage) -> BackendKind {
    match stage {
        VisualMaterializationStage::Cache => BackendKind::VisualCache,
        VisualMaterializationStage::OcrTiny => BackendKind::OcrTiny,
        VisualMaterializationStage::OcrMedium => BackendKind::OcrMedium,
        VisualMaterializationStage::GuiGrounding => BackendKind::GuiGrounding,
    }
}

/// 读取已验证 HWND 的屏幕矩形。
fn window_bounds(window: &WindowContext) -> Result<RECT, AutomationError> {
    let hwnd = HWND(window.handle as usize as *mut std::ffi::c_void);
    let mut bounds = RECT::default();
    // SAFETY: bounds 是同步 Win32 调用的独占输出，HWND 由 AppSession/前台上下文提供。
    unsafe { GetWindowRect(hwnd, &mut bounds) }
        .map(|_| bounds)
        .map_err(|error| AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::SendInput,
            message: format!("failed to read target window bounds: {error}"),
        })
}

/// 把视觉管线错误归因到真正失败的物化阶段，而不是笼统归因给 SendInput。
fn map_vision_error(backend: BackendKind, error: VisionError) -> AutomationError {
    match error {
        VisionError::CaptureUnavailable { message }
        | VisionError::WorkerUnavailable { message }
        | VisionError::OcrFailed { message }
        | VisionError::Protocol { message } => {
            AutomationError::BackendUnavailable { backend, message }
        }
        other => AutomationError::BackendFailed {
            backend,
            message: other.to_string(),
        },
    }
}
