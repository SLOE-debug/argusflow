use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_agent::{
    MaterializedTarget, MaterializedTargetValidator, PreparedTargetMaterializer, VisualBaseline,
    VisualMaterializationPlan, VisualMaterializationStage, VisualTargetBounds,
    VisualVerificationProvider, VisualVerificationResult, WindowContext,
};
use argusflow_core::{
    AutomationError, BackendKind, PreparedTargetLocator, VisualQuery, WindowIdentity,
};
use argusflow_vision::{
    PhysicalRect, SceneRefreshPolicy, VerificationOutcome, VisionError, VisionRuntime,
    VisualCondition, evaluate_visual_condition,
};
use async_trait::async_trait;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::GetWindowRect,
};

use super::surface_transform::SurfaceTransform;
use super::visual_query_target::{prepare_query, select_click_node};

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

/// Cache 物化前允许的轻量新帧复验时间，避免使用半秒前的旧坐标。
const CACHE_REVALIDATION_TIMEOUT: Duration = Duration::from_millis(75);

#[async_trait]
impl PreparedTargetMaterializer for WindowsVisualTargetMaterializer {
    fn available_stages(&self) -> Vec<VisualMaterializationStage> {
        let health = self.runtime.health();
        if !health.capture_ready {
            return Vec::new();
        }
        let mut stages = vec![VisualMaterializationStage::Cache];
        if health.worker_ready {
            stages.extend([
                VisualMaterializationStage::OcrTiny,
                VisualMaterializationStage::OcrSmall,
                VisualMaterializationStage::OcrMedium,
            ]);
        }
        stages
    }

    async fn materialize(
        &self,
        window: &WindowContext,
        locator: &PreparedTargetLocator,
        plan: &VisualMaterializationPlan,
    ) -> Result<MaterializedTarget, AutomationError> {
        let identity = WindowIdentity {
            handle: window.handle,
            process_id: window.process_id,
        };
        let query = prepare_query(locator)?;
        let legacy_region = match &query {
            argusflow_vision::PreparedVisionQuery::Legacy(query) => query.region,
            argusflow_vision::PreparedVisionQuery::Aql { .. } => None,
        };
        let mut last_error = None;
        for (stage_index, stage) in plan.stages.iter().enumerate() {
            let result = match stage {
                VisualMaterializationStage::Cache => {
                    let mut refresh = SceneRefreshPolicy::tiny();
                    refresh.normalized_query_region = legacy_region;
                    let topology_generation = match self
                        .runtime
                        .revalidate_cache(identity, CACHE_REVALIDATION_TIMEOUT)
                        .await
                    {
                        Ok(generation) => generation,
                        Err(error) => {
                            last_error = Some(map_vision_error(BackendKind::VisualCache, error));
                            continue;
                        }
                    };
                    refresh.max_age = CACHE_REVALIDATION_TIMEOUT;
                    refresh.topology_generation = topology_generation;
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
                    refresh.normalized_query_region = legacy_region;
                    self.runtime
                        .current_scene(identity, &refresh)
                        .await
                        .map_err(|error| map_vision_error(BackendKind::OcrTiny, error))
                }
                VisualMaterializationStage::OcrSmall => {
                    let mut refresh = SceneRefreshPolicy::small();
                    refresh.normalized_query_region = legacy_region;
                    self.runtime
                        .current_scene(identity, &refresh)
                        .await
                        .map_err(|error| map_vision_error(BackendKind::OcrSmall, error))
                }
                VisualMaterializationStage::OcrMedium => {
                    let mut refresh = SceneRefreshPolicy::medium();
                    refresh.normalized_query_region = legacy_region;
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
                    let node = match select_click_node(&scene, &query) {
                        Err(error @ AutomationError::TargetNotFound { .. }) => {
                            last_error = Some(error);
                            continue;
                        }
                        Err(error @ AutomationError::AmbiguousTarget { .. })
                            if has_later_ocr_stage(&plan.stages[stage_index + 1..]) =>
                        {
                            last_error = Some(error);
                            continue;
                        }
                        Ok(node) => node,
                        Err(error) => return Err(error),
                    };
                    let transform = SurfaceTransform::new_with_origin(
                        window_bounds(window)?,
                        scene.viewport,
                        scene.viewport_origin,
                    )?;
                    let mapped = transform.map_rect(node.bbox)?;
                    return Ok(MaterializedTarget {
                        window: window.clone(),
                        scene_id: scene.scene_id.get(),
                        frame_id: scene.frame_id.get(),
                        topology_generation: scene.topology_generation.get(),
                        bounds: mapped.bounds,
                        frame_bounds: VisualTargetBounds {
                            x: node.bbox.x,
                            y: node.bbox.y,
                            width: node.bbox.width,
                            height: node.bbox.height,
                        },
                        surface_bounds: mapped.surface_bounds,
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
        let mut refresh = SceneRefreshPolicy::small();
        refresh.force_refresh = true;
        refresh.max_age = Duration::ZERO;
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
        wait: argusflow_core::TargetWaitPolicy,
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
        let deadline = (wait.mode == argusflow_core::TargetWaitMode::Bounded)
            .then(|| tokio::time::Instant::now() + Duration::from_millis(wait.timeout_ms));
        let mut last_reason = "尚未得到动作后的新视觉场景".to_owned();
        loop {
            if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
                return Ok(VisualVerificationResult::Uncertain {
                    reason: format!("视觉后置条件观察超时：{last_reason}"),
                });
            }
            let mut refresh = SceneRefreshPolicy::medium();
            refresh.normalized_query_region = query.region;
            let current_result = match deadline {
                Some(deadline) => tokio::time::timeout_at(
                    deadline,
                    self.runtime.current_scene(identity, &refresh),
                )
                .await
                .map_err(|_| AutomationError::OutcomeUnknown {
                    backend: BackendKind::OcrMedium,
                    message: "视觉后置条件观察达到总截止时间".to_owned(),
                })
                .and_then(|result| {
                    result.map_err(|error| map_vision_error(BackendKind::OcrMedium, error))
                }),
                None => self
                    .runtime
                    .current_scene(identity, &refresh)
                    .await
                    .map_err(|error| map_vision_error(BackendKind::OcrMedium, error)),
            };
            let current = match current_result {
                Ok(current) => current,
                Err(error) if deadline.is_some() => {
                    last_reason = error.to_string();
                    let Some(deadline) = deadline else {
                        return Err(error);
                    };
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Ok(VisualVerificationResult::Uncertain {
                            reason: format!("视觉后置条件观察超时：{last_reason}"),
                        });
                    }
                    tokio::time::sleep(
                        Duration::from_millis(wait.poll_interval_ms.max(1)).min(remaining),
                    )
                    .await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let condition = VisualCondition::NewTextExistsSince {
                query: query.clone(),
                since_scene_id: previous.scene_id,
                region: None,
            };
            let verification =
                match evaluate_visual_condition(Some(&current), Some(&previous), &condition) {
                    VerificationOutcome::Confirmed { .. } => VisualVerificationResult::Confirmed,
                    VerificationOutcome::Rejected { reason } => {
                        last_reason = reason.clone();
                        VisualVerificationResult::Rejected { reason }
                    }
                    VerificationOutcome::Uncertain { reason } => {
                        last_reason = reason.clone();
                        VisualVerificationResult::Uncertain { reason }
                    }
                };
            if matches!(verification, VisualVerificationResult::Confirmed) || deadline.is_none() {
                return Ok(verification);
            }
            let Some(deadline) = deadline else {
                return Ok(verification);
            };
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(VisualVerificationResult::Uncertain {
                    reason: format!("视觉后置条件观察超时：{last_reason}"),
                });
            }
            tokio::time::sleep(Duration::from_millis(wait.poll_interval_ms.max(1)).min(remaining))
                .await;
        }
    }
}

#[async_trait]
impl MaterializedTargetValidator for WindowsVisualTargetMaterializer {
    async fn validate_before_input(
        &self,
        target: &MaterializedTarget,
    ) -> Result<(), AutomationError> {
        let identity = WindowIdentity {
            handle: target.window.handle,
            process_id: target.window.process_id,
        };
        self.runtime
            .validate_materialized_target(
                identity,
                target.scene_id,
                target.frame_id,
                target.topology_generation,
                PhysicalRect::new(
                    target.frame_bounds.x,
                    target.frame_bounds.y,
                    target.frame_bounds.width,
                    target.frame_bounds.height,
                )
                .ok_or_else(|| AutomationError::VisualTargetStale {
                    message: "visual target has empty frame-local bounds".to_owned(),
                })?,
            )
            .await
            .map_err(|error| match error {
                VisionError::SceneStale
                | VisionError::WindowIdentityChanged { .. }
                | VisionError::OcrCancelled { .. }
                | VisionError::FrameTimeout { .. } => AutomationError::VisualTargetStale {
                    message: error.to_string(),
                },
                other => map_vision_error(BackendKind::SendInput, other),
            })
    }
}

/// 将物化阶段转换为事实来源，供输入证据和错误定位使用。
const fn stage_backend(stage: VisualMaterializationStage) -> BackendKind {
    stage.backend_kind()
}

/// 判断剩余物化计划是否还能用更高精度 OCR 消解当前歧义。
fn has_later_ocr_stage(stages: &[VisualMaterializationStage]) -> bool {
    stages.iter().any(|stage| {
        matches!(
            stage,
            VisualMaterializationStage::OcrSmall | VisualMaterializationStage::OcrMedium
        )
    })
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
