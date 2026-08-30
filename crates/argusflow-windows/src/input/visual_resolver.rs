//! 多窗口 Vision Scene 到安全 SendInput 目标的物化。

use std::sync::Arc;

use argusflow_agent::{
    MaterializedTarget, MaterializedTargetValidator, PreparedTargetMaterialization,
    PreparedTargetMaterializer, VisualMaterializationPlan, VisualMaterializationStage,
    VisualTargetBounds, WindowContext,
};
use argusflow_core::{AutomationError, BackendKind, PreparedTargetLocator, WindowIdentity};
use argusflow_vision::{
    PhysicalRect, ResolvedTargetHandoffKey, VisionError, VisionQueryPlan, VisionRuntime,
    VisualNodeSource, WindowInventory, compile_vision_query,
};
use async_trait::async_trait;
use windows::Win32::Foundation::RECT;

use super::surface_transform::SurfaceTransform;

/// 基于共享 VisionRuntime 的 Windows 视觉目标物化器。
#[derive(Debug, Clone)]
pub struct WindowsVisualTargetMaterializer {
    /// 与视觉读取后端共享捕获、OCR 和 WindowScene 状态。
    runtime: Arc<VisionRuntime>,
    /// 枚举同一进程新增顶层窗口的平台注册表。
    inventory: Arc<dyn WindowInventory>,
}

impl WindowsVisualTargetMaterializer {
    /// 创建单一 Small→Medium 策略的物化器。
    pub fn new(runtime: Arc<VisionRuntime>, inventory: Arc<dyn WindowInventory>) -> Self {
        Self { runtime, inventory }
    }
}

#[async_trait]
impl PreparedTargetMaterializer for WindowsVisualTargetMaterializer {
    fn available_stages(&self) -> Vec<VisualMaterializationStage> {
        let health = self.runtime.health();
        if health.capture_ready && health.worker_ready {
            vec![VisualMaterializationStage::OcrSmall]
        } else {
            Vec::new()
        }
    }

    fn prepare(
        &self,
        locator: &PreparedTargetLocator,
    ) -> Result<Arc<dyn PreparedTargetMaterialization>, AutomationError> {
        let PreparedTargetLocator::Query { query, source } = locator else {
            return Err(AutomationError::BackendUnavailable {
                backend: BackendKind::OcrSmall,
                message: "Vision materialization requires a prepared AQL query".to_owned(),
            });
        };
        let plan =
            compile_vision_query(query).map_err(|_error| AutomationError::ActionUnsupported {
                backend: BackendKind::OcrSmall,
                query: source.clone(),
                semantic_matches: 0,
                required: argusflow_core::ActionCapability::Activate,
            })?;
        let handoff_key = ResolvedTargetHandoffKey::from_query(query);
        Ok(Arc::new(PreparedVisionAqlMaterialization {
            runtime: self.runtime.clone(),
            inventory: self.inventory.clone(),
            query: Arc::new(plan),
            handoff_key,
            source: source.clone(),
        }))
    }
}

/// 已编译一次、可被等待与 stale 重试复用的 Vision AQL 点击计划。
#[derive(Debug)]
struct PreparedVisionAqlMaterialization {
    /// 与读取后端共享的 Scene/OCR runtime。
    runtime: Arc<VisionRuntime>,
    /// 同一进程全部可见窗口注册表。
    inventory: Arc<dyn WindowInventory>,
    /// 已预编译正则与空间选择语义的计划。
    query: Arc<VisionQueryPlan>,
    /// 已绑定参数的查询身份，用于消费紧邻读取节点交接的目标。
    handoff_key: ResolvedTargetHandoffKey,
    /// 诊断使用的原始 AQL。
    source: String,
}

#[async_trait]
impl PreparedTargetMaterialization for PreparedVisionAqlMaterialization {
    async fn materialize(
        &self,
        window: &WindowContext,
        _plan: &VisualMaterializationPlan,
        trace_context: Option<&argusflow_core::RunTraceContext>,
    ) -> Result<MaterializedTarget, AutomationError> {
        let handoff_target = match trace_context {
            Some(context) => {
                self.runtime
                    .take_resolved_target_handoff(
                        context,
                        window.process_id,
                        &self.handoff_key,
                        0.80,
                    )
                    .await
            }
            None => None,
        };
        let target = match handoff_target {
            Some(target) => target,
            None => {
                self.runtime
                    .resolve_query(
                        self.inventory.as_ref(),
                        window.process_id,
                        &self.query,
                        &self.source,
                        0.80,
                        trace_context,
                    )
                    .await?
            }
        };
        let selected_window = WindowContext {
            handle: target.window.identity.handle,
            process_id: target.window.identity.process_id,
        };
        // Owned popup/tool window 能提供正确的捕获表面，但经常拒绝 SetForegroundWindow；
        // 将其 owner 保存为显式激活降级目标，避免混淆坐标表面与前台窗口。
        let activation_fallback = target
            .window
            .owner_handle
            .filter(|owner_handle| *owner_handle != selected_window.handle)
            .map(|owner_handle| WindowContext {
                handle: owner_handle,
                process_id: target.window.identity.process_id,
            });
        let transform = SurfaceTransform::new_with_origin(
            rect_from_physical(target.window.screen_bounds),
            target.scene.viewport,
            target.scene.viewport_origin,
        )?;
        let mapped = transform.map_rect(target.node.bbox)?;
        Ok(MaterializedTarget {
            window: selected_window,
            activation_fallback,
            scene_id: target.scene.scene_id.get(),
            frame_id: target.scene.frame_id.get(),
            topology_generation: target.scene.topology_generation.get(),
            bounds: mapped.bounds,
            frame_bounds: VisualTargetBounds {
                x: target.node.bbox.x,
                y: target.node.bbox.y,
                width: target.node.bbox.width,
                height: target.node.bbox.height,
            },
            surface_bounds: mapped.surface_bounds,
            confidence: target.node.confidence,
            safe_point: mapped.safe_point,
            source_backend: source_backend(target.node.source),
        })
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
        let target_region = PhysicalRect::new(
            target.frame_bounds.x,
            target.frame_bounds.y,
            target.frame_bounds.width,
            target.frame_bounds.height,
        )
        .ok_or_else(|| AutomationError::VisualTargetStale {
            message: "visual target has empty frame-local bounds".to_owned(),
        })?;
        self.runtime
            .validate_materialized_target(
                identity,
                target.scene_id,
                target.frame_id,
                target.topology_generation,
                target_region,
            )
            .await
            .map_err(map_validation_error)
    }
}

/// 将窗口物理矩形转换为现有坐标投影输入。
fn rect_from_physical(bounds: PhysicalRect) -> RECT {
    RECT {
        left: bounds.x,
        top: bounds.y,
        right: bounds
            .right()
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        bottom: bounds
            .bottom()
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    }
}

/// Small 与内部 Medium 升级都属于同一个对外 Vision Backend。
const fn source_backend(_source: VisualNodeSource) -> BackendKind {
    BackendKind::OcrSmall
}

/// 输入提交点只把陈旧性错误映射为可重新物化目标。
fn map_validation_error(error: VisionError) -> AutomationError {
    match error {
        VisionError::CaptureUnavailable { .. }
        | VisionError::SceneStale
        | VisionError::WindowIdentityChanged { .. }
        | VisionError::OcrCancelled { .. }
        | VisionError::FrameTimeout { .. }
        | VisionError::FrameUnstable { .. }
        | VisionError::InvalidRoi { .. } => AutomationError::VisualTargetStale {
            message: error.to_string(),
        },
        other => AutomationError::BackendFailed {
            backend: BackendKind::SendInput,
            message: other.to_string(),
        },
    }
}
