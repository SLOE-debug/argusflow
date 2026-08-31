//! 单一工作流控制路径的节点执行、输出发布与事件编排。

use std::{collections::HashMap, sync::Arc};

use argusflow_core::{ExecutionEventKind, ExecutionEventPayload, WorkflowEdge, WorkflowNode};

use super::{
    engine::{ExecutionEventSink, emit_traced_event},
    execution_events::build_event,
    node_execution::catch_node_unwind,
    run_context::RunContext,
    scheduler::ResourceScheduler,
};
use crate::{
    RuntimeError,
    node_registry::{NodeFlow, PreparedNode},
    validation::validator::PreparedWorkflow,
    value_runtime::publish_outcome,
};

/// 单节点执行共享的只读运行设施，避免调用边界出现长参数列表。
#[derive(Clone, Copy)]
struct NodeExecutionEnvironment<'runtime> {
    /// 当前已经完成校验与冻结的工作流。
    workflow: &'runtime PreparedWorkflow,
    /// 实时事件接收器。
    sink: &'runtime Arc<dyn ExecutionEventSink>,
    /// 可选持久化 Trace 会话。
    trace: Option<&'runtime Arc<dyn crate::RunTraceSession>>,
    /// 跨 RunWorld 资源调度器。
    scheduler: &'runtime ResourceScheduler,
}

/// 沿唯一命中控制路径执行节点，并把原生结果原子发布到 RunContext 数据面。
pub(super) async fn execute_path(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &mut RunContext,
    sequence: &mut u64,
    scheduler: &ResourceScheduler,
) -> Result<(), RuntimeError> {
    let nodes = index_nodes(workflow);
    let outgoing = index_edges(workflow);
    let mut current_id = start_node_id(workflow);

    while let Some(node_id) = current_id {
        let node = nodes.get(node_id).ok_or_else(|| {
            RuntimeError::ExecutionInvariant(format!(
                "node '{node_id}' disappeared after validation"
            ))
        })?;
        let prepared = workflow.nodes.get(node_id).ok_or_else(|| {
            RuntimeError::ExecutionInvariant(format!(
                "node '{node_id}' has no prepared execution after validation"
            ))
        })?;
        record_node_started(workflow, sink, trace, context, sequence, node, prepared)?;
        let execution = execute_node(
            NodeExecutionEnvironment {
                workflow,
                sink,
                trace,
                scheduler,
            },
            context,
            sequence,
            node,
            prepared,
        )
        .await?;
        let selected_branch = execution.branch.clone();
        let termination = execution.termination.clone();
        publish_node_result(workflow, sink, trace, context, sequence, node, execution)?;

        if let Some(termination) = termination {
            if let Some(trace) = trace {
                let failure = format!("{}: {}", termination.code, termination.message);
                trace.finish(crate::RunStatus::Failed, Some(&node.id), Some(&failure));
            }
            return Err(RuntimeError::WorkflowFailure {
                code: termination.code,
                message: termination.message,
            });
        }

        let next_edge = select_next_edge(
            prepared.as_ref(),
            node_id,
            &outgoing,
            selected_branch.as_ref(),
        );
        current_id = traverse_edge(workflow, sink, trace, context, sequence, next_edge)?;
    }
    Ok(())
}

/// 为节点执行建立只读 ID 索引。
fn index_nodes(workflow: &PreparedWorkflow) -> HashMap<&str, &WorkflowNode> {
    workflow
        .definition
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect()
}

/// 为后继选择建立只读出边索引。
fn index_edges(workflow: &PreparedWorkflow) -> HashMap<&str, Vec<&WorkflowEdge>> {
    let mut outgoing: HashMap<&str, Vec<&WorkflowEdge>> = HashMap::new();
    for edge in &workflow.definition.edges {
        outgoing.entry(edge.source.as_str()).or_default().push(edge);
    }
    outgoing
}

/// 从 PreparedNode 的强类型控制流声明中解析唯一 Start。
fn start_node_id(workflow: &PreparedWorkflow) -> Option<&str> {
    workflow
        .definition
        .nodes
        .iter()
        .find(|node| {
            workflow
                .nodes
                .get(&node.id)
                .is_some_and(|prepared| matches!(prepared.flow(), NodeFlow::Start))
        })
        .map(|node| node.id.as_str())
}

/// 记录冻结输入并发出节点开始事件。
fn record_node_started(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    prepared: &Arc<dyn PreparedNode>,
) -> Result<(), RuntimeError> {
    if let Some(trace) = trace {
        trace.record_resolved_inputs(*sequence, &node.id, prepared.as_ref(), context);
    }
    emit_traced_event(
        sink,
        trace,
        build_event(
            context.run_id,
            workflow.definition.id,
            sequence,
            Some(node.id.clone()),
            None,
            ExecutionEventKind::NodeStarted,
            Some(prepared.label()),
            None,
        ),
        &workflow.source_map,
    )
}

/// 在资源访问锁内执行一个 PreparedNode，并统一报告失败。
async fn execute_node(
    environment: NodeExecutionEnvironment<'_>,
    context: &mut RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    prepared: &Arc<dyn PreparedNode>,
) -> Result<crate::NodeExecution, RuntimeError> {
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
            record_node_failure(
                environment.workflow,
                environment.sink,
                environment.trace,
                context,
                sequence,
                node,
                &error,
            )?;
            Err(error)
        }
    }
}

/// 原子发布节点值输出并发出节点自定义事件、输出事件和成功事件。
fn publish_node_result(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &mut RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    execution: crate::NodeExecution,
) -> Result<(), RuntimeError> {
    let published = publish_outcome(context, &node.id, execution.outcome, &node.output_bindings);
    let published = match published {
        Ok(outcome) => outcome,
        Err(error) => {
            record_node_failure(workflow, sink, trace, context, sequence, node, &error)?;
            return Err(error);
        }
    };
    let output_names = published.outputs.keys().cloned().collect::<Vec<_>>();
    if let Some(trace) = trace {
        trace.record_outputs(*sequence, &node.id, &published);
    }
    context.record_outcome(node.id.clone(), published);
    for event in execution.events {
        emit_traced_event(
            sink,
            trace,
            build_event(
                context.run_id,
                workflow.definition.id,
                sequence,
                Some(node.id.clone()),
                None,
                event.kind,
                event.message,
                event.payload,
            ),
            &workflow.source_map,
        )?;
    }
    if !output_names.is_empty() {
        emit_traced_event(
            sink,
            trace,
            build_event(
                context.run_id,
                workflow.definition.id,
                sequence,
                Some(node.id.clone()),
                None,
                ExecutionEventKind::NodeOutputProduced,
                Some(format!("已产生 {} 个公开值输出", output_names.len())),
                Some(ExecutionEventPayload::NodeOutputsProduced { output_names }),
            ),
            &workflow.source_map,
        )?;
    }
    emit_traced_event(
        sink,
        trace,
        build_event(
            context.run_id,
            workflow.definition.id,
            sequence,
            Some(node.id.clone()),
            None,
            ExecutionEventKind::NodeSucceeded,
            None,
            None,
        ),
        &workflow.source_map,
    )
}

/// 统一发出节点失败并终结 Trace。
fn record_node_failure(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &RunContext,
    sequence: &mut u64,
    node: &WorkflowNode,
    error: &RuntimeError,
) -> Result<(), RuntimeError> {
    emit_traced_event(
        sink,
        trace,
        build_event(
            context.run_id,
            workflow.definition.id,
            sequence,
            Some(node.id.clone()),
            None,
            ExecutionEventKind::NodeFailed,
            Some(error.to_string()),
            None,
        ),
        &workflow.source_map,
    )?;
    if let Some(trace) = trace {
        trace.finish(
            crate::RunStatus::Failed,
            Some(&node.id),
            Some(&error.to_string()),
        );
    }
    Ok(())
}

/// 根据分支值选择唯一后继边。
fn select_next_edge<'edge>(
    node: &dyn PreparedNode,
    node_id: &str,
    outgoing: &'edge HashMap<&str, Vec<&'edge WorkflowEdge>>,
    selected_branch: Option<&argusflow_core::ControlPortId>,
) -> Option<&'edge WorkflowEdge> {
    if matches!(node.flow(), NodeFlow::End) {
        return None;
    }
    outgoing
        .get(node_id)
        .into_iter()
        .flatten()
        .find(|edge| edge.branch.as_ref() == selected_branch)
        .copied()
}

/// 发出边遍历事件并返回下一节点 ID。
fn traverse_edge<'edge>(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &RunContext,
    sequence: &mut u64,
    edge: Option<&'edge WorkflowEdge>,
) -> Result<Option<&'edge str>, RuntimeError> {
    let Some(edge) = edge else { return Ok(None) };
    emit_traced_event(
        sink,
        trace,
        build_event(
            context.run_id,
            workflow.definition.id,
            sequence,
            None,
            Some(edge.id.clone()),
            ExecutionEventKind::EdgeTraversed,
            Some(format!("{} → {}", edge.source, edge.target)),
            None,
        ),
        &workflow.source_map,
    )?;
    Ok(Some(edge.target.as_str()))
}
