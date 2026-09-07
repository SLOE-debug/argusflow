//! 复用窗口 capture/OCR Scene 的点反查，不另建 OCR 或图像执行栈。

use argusflow_core::{
    ElementRole, ElementSemantics, FieldSensitivity, InspectedEntity, InspectionContext,
    InspectionFailure, InspectionProbe, InspectionRect, TargetInspector,
};
use async_trait::async_trait;

use crate::{SceneRefreshPolicy, VisionRuntime};

#[async_trait]
impl TargetInspector for VisionRuntime {
    async fn inspect(
        &self,
        context: &InspectionContext,
        probe: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        let InspectionProbe::Point(point) = probe else {
            return Err(InspectionFailure::NoElement);
        };
        let policy = SceneRefreshPolicy {
            force_refresh: true,
            ..SceneRefreshPolicy::small()
        };
        let scene = self
            .current_scene(context.window, &policy)
            .await
            .map_err(|_| InspectionFailure::Unavailable)?;
        if scene.window != context.window {
            return Err(InspectionFailure::ContextChanged);
        }
        // OCR BBox 是 capture frame 本地物理坐标；必须使用 Scene 实际原点而非窗口外框。
        let local_x = i64::from(point.x) - i64::from(scene.viewport_origin.x);
        let local_y = i64::from(point.y) - i64::from(scene.viewport_origin.y);
        let node = scene
            .nodes
            .iter()
            .filter(|node| {
                node.confidence >= 0.5
                    && local_x >= i64::from(node.bbox.x)
                    && local_y >= i64::from(node.bbox.y)
                    && local_x < i64::from(node.bbox.x) + i64::from(node.bbox.width)
                    && local_y < i64::from(node.bbox.y) + i64::from(node.bbox.height)
            })
            .min_by_key(|node| u64::from(node.bbox.width) * u64::from(node.bbox.height))
            .ok_or(InspectionFailure::NoElement)?;
        Ok(InspectedEntity {
            identity: format!("scene:{}:{}", scene.scene_id.get(), node.id.get()),
            semantics: ElementSemantics {
                role: Some(ElementRole::Text),
                name: Some(node.normalized_text.clone()),
                ..ElementSemantics::default()
            },
            ancestors: Vec::new(),
            bounds: InspectionRect {
                x: f64::from(scene.viewport_origin.x) + f64::from(node.bbox.x),
                y: f64::from(scene.viewport_origin.y) + f64::from(node.bbox.y),
                width: f64::from(node.bbox.width),
                height: f64::from(node.bbox.height),
            },
            editable: false,
            sensitivity: FieldSensitivity::Unknown,
            browser_session: None,
            page_url: None,
            confidence: node.confidence.min(0.7),
        })
    }
}
