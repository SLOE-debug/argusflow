//! 多窗口 Vision Scene 到安全 SendInput 目标的物化。

use std::sync::Arc;

use argusflow_agent::{
    MaterializedTarget, MaterializedTargetValidator, PreparedTargetMaterializer,
    VisualMaterializationPlan, VisualMaterializationStage, VisualTargetBounds, WindowContext,
};
use argusflow_core::{AutomationError, BackendKind, PreparedTargetLocator, WindowIdentity};
use argusflow_vision::{
    PhysicalRect, VisionError, VisionRuntime, VisualNodeSource, WindowInventory,
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

    async fn materialize(
        &self,
        window: &WindowContext,
        locator: &PreparedTargetLocator,
        _plan: &VisualMaterializationPlan,
        trace_context: Option<&argusflow_core::RunTraceContext>,
    ) -> Result<MaterializedTarget, AutomationError> {
        let PreparedTargetLocator::Visual { query } = locator else {
            return Err(AutomationError::BackendUnavailable {
                backend: BackendKind::OcrSmall,
                message: "Vision MVP accepts only an explicit visual text locator".to_owned(),
            });
        };
        let target = self
            .runtime
            .resolve_text(
                self.inventory.as_ref(),
                window.process_id,
                query,
                0.80,
                trace_context,
            )
            .await?;
        let selected_window = WindowContext {
            handle: target.window.identity.handle,
            process_id: target.window.identity.process_id,
        };
        let transform = SurfaceTransform::new_with_origin(
            rect_from_physical(target.window.screen_bounds),
            target.scene.viewport,
            target.scene.viewport_origin,
        )?;
        let mapped = transform.map_rect(target.node.bbox)?;
        Ok(MaterializedTarget {
            window: selected_window,
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
        VisionError::SceneStale
        | VisionError::WindowIdentityChanged { .. }
        | VisionError::OcrCancelled { .. }
        | VisionError::FrameTimeout { .. } => AutomationError::VisualTargetStale {
            message: error.to_string(),
        },
        other => AutomationError::BackendFailed {
            backend: BackendKind::SendInput,
            message: other.to_string(),
        },
    }
}
