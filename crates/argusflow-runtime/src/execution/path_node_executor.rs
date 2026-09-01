//! 单个普通节点的资源调度、输出发布和生命周期事件。

use std::sync::Arc;

use argusflow_core::{ExecutionEventKind, ExecutionEventPayload, WorkflowNode};

use super::{
    loop_runtime::LoopFrame,
    node_execution::catch_node_unwind,
    path_runner::{NodeExecutionEnvironment, emit_path_event},
    run_context::RunContext,
};
use crate::{
    NodeExecution, RuntimeError, node_registry::PreparedNode, value_runtime::publish_outcome,
};

/// 记录冻结输入并发出节点开始事件。
pub(super) fn record_node_started(
    environment: NodeExecutionEnvironment<'_>,
    context: &RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    prepared: &Arc<dyn PreparedNode>,
    loop_stack: &[LoopFrame],
) -> Result<(), RuntimeError> {
    if let Some(trace) = environment.trace {
        trace.record_resolved_inputs(*sequence, &node.id, prepared.as_ref(), context);
    }
    let event = super::execution_events::build_event(
        context.run_id,
        environment.workflow.definition.id,
        sequence,
        Some(node.id.clone()),
        None,
        ExecutionEventKind::NodeStarted,
        Some(prepared.label()),
        None,
    );
    emit_path_event(environment, event, loop_stack)
}

/// 在资源访问锁内执行一个 PreparedNode，并统一报告失败。
pub(super) async fn execute_node(
    environment: NodeExecutionEnvironment<'_>,
    context: &mut RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    prepared: &Arc<dyn PreparedNode>,
    loop_stack: &[LoopFrame],
) -> Result<NodeExecution, RuntimeError> {
    let access = prepared.access_set(&node.id, context)?;
    let _access_guard = environment.scheduler.acquire(access).await;
    let result = catch_node_unwind(prepared.execute(
        &node.id,
        &environment.workflow.definition.permissions,
        context,
    ))
    .await
    .map_err(|message| RuntimeError::NodeExecution {
        message: format!("节点 '{}' 执行异常：{message}", node.id),
    })
    .and_then(|result| result);
    match result {
        Ok(execution) => Ok(execution),
        Err(error) => {
            record_node_failure(environment, context, sequence, node, &error, loop_stack)?;
            Err(error)
        }
    }
}

/// 原子发布节点值输出并发出节点自定义事件、输出事件和成功事件。
pub(super) fn publish_node_result(
    environment: NodeExecutionEnvironment<'_>,
    context: &mut RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    execution: NodeExecution,
    loop_stack: &[LoopFrame],
) -> Result<(), RuntimeError> {
    let published =
        match publish_outcome(context, &node.id, execution.outcome, &node.output_bindings) {
            Ok(outcome) => outcome,
            Err(error) => {
                record_node_failure(environment, context, sequence, node, &error, loop_stack)?;
                return Err(error);
            }
        };
    let output_names = published.outputs.keys().cloned().collect::<Vec<_>>();
    if let Some(trace) = environment.trace {
        trace.record_outputs(*sequence, &node.id, &published);
    }
    context.record_outcome(node.id.clone(), published);
    for node_event in execution.events {
        let event = super::execution_events::build_event(
            context.run_id,
            environment.workflow.definition.id,
            sequence,
            Some(node.id.clone()),
            None,
            node_event.kind,
            node_event.message,
            node_event.payload,
        );
        emit_path_event(environment, event, loop_stack)?;
    }
    if !output_names.is_empty() {
        let event = super::execution_events::build_event(
            context.run_id,
            environment.workflow.definition.id,
            sequence,
            Some(node.id.clone()),
            None,
            ExecutionEventKind::NodeOutputProduced,
            Some(format!("已产生 {} 个公开值输出", output_names.len())),
            Some(ExecutionEventPayload::NodeOutputsProduced { output_names }),
        );
        emit_path_event(environment, event, loop_stack)?;
    }
    let event = super::execution_events::build_event(
        context.run_id,
        environment.workflow.definition.id,
        sequence,
        Some(node.id.clone()),
        None,
        ExecutionEventKind::NodeSucceeded,
        None,
        None,
    );
    emit_path_event(environment, event, loop_stack)
}

/// 统一发出节点失败并终结 Trace。
fn record_node_failure(
    environment: NodeExecutionEnvironment<'_>,
    context: &RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    error: &RuntimeError,
    loop_stack: &[LoopFrame],
) -> Result<(), RuntimeError> {
    let event = super::execution_events::build_event(
        context.run_id,
        environment.workflow.definition.id,
        sequence,
        Some(node.id.clone()),
        None,
        ExecutionEventKind::NodeFailed,
        Some(error.to_string()),
        None,
    );
    emit_path_event(environment, event, loop_stack)?;
    if let Some(trace) = environment.trace {
        trace.finish(
            crate::RunStatus::Failed,
            Some(&node.id),
            Some(&error.to_string()),
        );
    }
    Ok(())
}
