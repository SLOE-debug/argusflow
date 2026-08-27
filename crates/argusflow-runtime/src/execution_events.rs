use std::sync::Arc;

use argusflow_core::{
    ExecutionComponentFrame, ExecutionEvent, ExecutionEventKind, ExecutionEventPayload,
};
use uuid::Uuid;

use crate::{ComponentSourceMap, ExecutionEventSink, RuntimeError};

/// 将事件交付错误统一映射到 RuntimeError，并恢复组件实例来源。
pub(crate) fn emit_event(
    sink: &Arc<dyn ExecutionEventSink>,
    mut event: ExecutionEvent,
    source_map: &ComponentSourceMap,
) -> Result<(), RuntimeError> {
    if let Some(expanded_node_id) = event.node_id.clone()
        && let Some(path) = source_map.get(&expanded_node_id)
        && let Some(root) = path.first()
    {
        event.node_id = Some(root.instance_node_id.clone());
        event.expanded_node_id = Some(expanded_node_id);
        event.component_path = path
            .iter()
            .map(|frame| ExecutionComponentFrame {
                instance_node_id: frame.instance_node_id.clone(),
                component_id: frame.component_id.as_uuid(),
                component_version: frame.component_version.as_str().to_owned(),
                inner_node_id: frame.inner_node_id.clone(),
            })
            .collect();
    }
    sink.emit(event).map_err(RuntimeError::EventSink)
}

/// 构造严格递增序号的执行事件。
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_event(
    run_id: Uuid,
    workflow_id: Uuid,
    sequence: &mut u64,
    node_id: Option<String>,
    edge_id: Option<String>,
    kind: ExecutionEventKind,
    message: Option<String>,
    payload: Option<ExecutionEventPayload>,
) -> ExecutionEvent {
    let event = ExecutionEvent {
        run_id,
        workflow_id,
        sequence: *sequence,
        node_id,
        expanded_node_id: None,
        component_path: Vec::new(),
        edge_id,
        kind,
        message,
        payload,
    };
    *sequence += 1;
    event
}
