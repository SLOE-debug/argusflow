//! 将固定页面检查函数结果转换为无 DOM handle 的语义快照。

use super::geometry::{ViewportMetrics, ViewportTransform};
use crate::cdp::CdpPageSession;
use argusflow_core::{
    ElementSemantics, FieldSensitivity, InspectedEntity, InspectionFailure, InspectionRect,
    ResourceId, sensitive_field_metadata,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// 固定结果结构；不反序列化任意属性或控件值。
#[derive(Deserialize)]
struct PageEntity {
    /// 用于与现有 AQL 执行器保持一致的 DOM 属性。
    semantics: ElementSemantics,
    /// 有限稳定祖先链。
    ancestors: Vec<ElementSemantics>,
    /// CSS viewport 矩形。
    bounds: InspectionRect,
    /// 当前元素是否可编辑。
    editable: bool,
    /// 页面从 type/autocomplete/字段元数据读取的敏感性。
    sensitive: bool,
    /// CSS bounds 是否属于已验证的顶层 viewport；与 selector 可执行性无关。
    top_level_viewport: bool,
}

pub(super) async fn inspect(
    page: &CdpPageSession,
    object_id: &str,
    resource: ResourceId,
    transform: &ViewportTransform,
    metrics: &ViewportMetrics,
) -> Result<InspectedEntity, InspectionFailure> {
    let result = page
        .command(
            "Runtime.callFunctionOn",
            json!({
                "objectId": object_id, "functionDeclaration": include_str!("inspect.js"),
                "returnByValue": true,
            }),
        )
        .await
        .map_err(|_| InspectionFailure::Unavailable)?;
    let value = result
        .pointer("/result/value")
        .cloned()
        .ok_or(InspectionFailure::NoElement)?;
    let entity: PageEntity =
        serde_json::from_value(value).map_err(|_| InspectionFailure::NoElement)?;
    if !entity.top_level_viewport {
        return Err(InspectionFailure::InvalidGeometry);
    }
    if !entity.bounds.is_valid() {
        return Err(InspectionFailure::NoElement);
    }
    let sensitive = entity.sensitive
        || [
            &entity.semantics.name,
            &entity.semantics.stable_id,
            &entity.semantics.test_id,
        ]
        .into_iter()
        .flatten()
        .any(|value| sensitive_field_metadata(value));
    // 不在共享观察层隐式遮盖，调用方按会话策略处理敏感性。
    let node = page
        .command(
            "DOM.describeNode",
            json!({ "objectId": object_id, "depth": 0 }),
        )
        .await
        .map_err(|_| InspectionFailure::NoElement)?;
    let node_id = node
        .pointer("/node/backendNodeId")
        .and_then(Value::as_u64)
        .ok_or(InspectionFailure::NoElement)?;
    Ok(InspectedEntity {
        identity: format!("{}:{node_id}", page.target_id()),
        semantics: entity.semantics,
        ancestors: entity.ancestors,
        bounds: transform.screen_bounds(entity.bounds),
        editable: entity.editable,
        sensitivity: if sensitive {
            FieldSensitivity::Sensitive
        } else {
            FieldSensitivity::Normal
        },
        browser_session: Some(resource),
        page_url: Some(metrics.url.clone()),
        confidence: 0.95,
    })
}
