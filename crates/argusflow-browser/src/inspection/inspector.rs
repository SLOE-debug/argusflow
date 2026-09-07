//! Managed CDP 的坐标/焦点检查；不创建连接、不附加任意 Chromium。

use argusflow_core::{
    InspectedEntity, InspectionContext, InspectionFailure, InspectionProbe, TargetInspector,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{
    geometry::{self, ViewportMetrics, ViewportTransform},
    lease, page,
};
use crate::CdpRuntime;

#[async_trait]
impl TargetInspector for CdpRuntime {
    async fn inspect(
        &self,
        context: &InspectionContext,
        probe: InspectionProbe,
    ) -> Result<InspectedEntity, InspectionFailure> {
        let (resource, page) = self
            .inspection_session(
                context.window.process_id,
                context.executable_path.as_deref(),
            )
            .ok_or(InspectionFailure::UnmanagedWindow)?;
        inspect_attached(resource, page, context, probe).await
    }
}

/// 在已经由 runtime 验证所有权的页面上执行只读反查，供协议测试复用。
pub(super) async fn inspect_attached(
    resource: argusflow_core::ResourceId,
    page: std::sync::Arc<crate::cdp::CdpPageSession>,
    context: &InspectionContext,
    probe: InspectionProbe,
) -> Result<InspectedEntity, InspectionFailure> {
    // 同进程可有多个窗口/标签；必须同时证明原生键盘窗口与已附加 document 拥有焦点。
    if !context.has_keyboard_focus {
        return Err(InspectionFailure::ContextChanged);
    }
    let result = page
        .command(
            "Runtime.evaluate",
            json!({
                "expression": geometry::METRICS_SCRIPT, "returnByValue": true,
            }),
        )
        .await
        .map_err(|_| InspectionFailure::Unavailable)?;
    let metrics: ViewportMetrics = serde_json::from_value(
        result
            .pointer("/result/value")
            .cloned()
            .ok_or(InspectionFailure::InvalidGeometry)?,
    )
    .map_err(|_| InspectionFailure::InvalidGeometry)?;
    let transform = ViewportTransform::new(context, &metrics)?;
    let lease = lease::InspectionLease::new(page.clone());
    let object_id = match probe {
        InspectionProbe::Point(point) => {
            let (x, y) = transform.to_css(point)?;
            let hit = page
                .command(
                    "DOM.getNodeForLocation",
                    json!({ "x": x, "y": y,
                    "includeUserAgentShadowDOM": false, "ignorePointerEventsNone": false }),
                )
                .await
                .map_err(|_| InspectionFailure::NoElement)?;
            let backend_id = hit
                .get("backendNodeId")
                .and_then(Value::as_u64)
                .ok_or(InspectionFailure::NoElement)?;
            let resolved = page
                .command(
                    "DOM.resolveNode",
                    json!({ "backendNodeId": backend_id,
                    "objectGroup": lease.group() }),
                )
                .await
                .map_err(|_| InspectionFailure::NoElement)?;
            resolved
                .pointer("/object/objectId")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(InspectionFailure::NoElement)?
        }
        InspectionProbe::Focus => {
            let result = page
                .command(
                    "Runtime.evaluate",
                    json!({
                        "expression": "document.activeElement", "objectGroup": lease.group(),
                    }),
                )
                .await
                .map_err(|_| InspectionFailure::Unavailable)?;
            result
                .pointer("/result/objectId")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(InspectionFailure::NoElement)?
        }
    };
    page::inspect(&page, &object_id, resource, &transform, &metrics).await
}
