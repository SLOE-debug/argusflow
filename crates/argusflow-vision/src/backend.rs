//! 单一 Vision Backend：进程窗口枚举、Small OCR 与 Medium 内部升级。

use std::sync::Arc;

use argusflow_agent::{ExecutionContext, ObservationBackend, ObservationBackendError};
use argusflow_core::{
    BackendKind, CoordinateSpace as EntityCoordinateSpace, EntityBounds, EntityObservation,
    EntitySnapshot, EntitySource, ObservationRequest, ObservationUnknownReason,
};
use async_trait::async_trait;

use crate::{
    SceneRefreshPolicy, VisionRuntime, WindowInventory, compile_vision_query, evaluate_vision_query,
};

/// 对外只暴露一个候选，OCR 档位升级完全封装在 VisionRuntime 内部。
#[derive(Debug, Clone)]
pub struct VisionBackend {
    /// 捕获、增量 Scene 和 OCR 策略运行时。
    runtime: Arc<VisionRuntime>,
    /// 平台窗口注册表。
    inventory: Arc<dyn WindowInventory>,
}

impl VisionBackend {
    /// 创建绑定共享 Runtime 与 Windows 窗口注册表的视觉读取后端。
    pub fn new(runtime: Arc<VisionRuntime>, inventory: Arc<dyn WindowInventory>) -> Self {
        Self { runtime, inventory }
    }

    /// 返回共享 Runtime。
    pub fn runtime(&self) -> Arc<VisionRuntime> {
        self.runtime.clone()
    }
}

#[async_trait]
impl ObservationBackend for VisionBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::OcrSmall
    }

    async fn observe(
        &self,
        request: &ObservationRequest,
        context: &ExecutionContext,
    ) -> Result<Vec<EntityObservation>, ObservationBackendError> {
        let process_id = context
            .foreground_window
            .as_ref()
            .map(|window| window.process_id)
            .ok_or(ObservationBackendError::Unavailable { retryable: true })?;
        let plans = argusflow_query::observation_selectors(&request.query)
            .into_iter()
            .map(compile_vision_query)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ObservationBackendError::Unsupported)?;

        // Observe 必须在调用开始后取得完整新帧，不能把旧缓存中的“未命中”当作确定不存在。
        let mut policy = SceneRefreshPolicy::small();
        policy.force_refresh = true;
        policy.force_full_ocr = true;
        let scene = self
            .runtime
            .current_app_scene(
                self.inventory.as_ref(),
                process_id,
                &policy,
                context.trace_context.as_ref(),
            )
            .await
            .map_err(|_| ObservationBackendError::Unavailable { retryable: true })?;

        plans
            .iter()
            .map(|plan| {
                let result =
                    evaluate_vision_query(&scene, plan, &request.source).map_err(|_| {
                        ObservationBackendError::Unknown {
                            reason: ObservationUnknownReason::InvalidResponse,
                            retryable: false,
                        }
                    })?;
                let entities = result
                    .matches
                    .into_iter()
                    .map(|candidate| {
                        let bounds = candidate.node.bbox;
                        EntitySnapshot {
                            name: Some(candidate.node.raw_text.clone()),
                            text: Some(candidate.node.raw_text.clone()),
                            value: None,
                            role: Some("text".to_owned()),
                            bounds: Some(EntityBounds {
                                space: EntityCoordinateSpace::ScreenPhysical,
                                x: f64::from(candidate.scene.viewport_origin.x + bounds.x),
                                y: f64::from(candidate.scene.viewport_origin.y + bounds.y),
                                width: f64::from(bounds.width),
                                height: f64::from(bounds.height),
                            }),
                            confidence: Some(candidate.node.confidence),
                            source: EntitySource::Ocr,
                        }
                    })
                    .collect();
                Ok(EntityObservation {
                    entities,
                    complete: true,
                })
            })
            .collect()
    }
}
