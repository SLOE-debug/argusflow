//! 多作用域控制路径的显式栈执行、输出发布与事件编排。

use std::{collections::HashMap, sync::Arc};

use argusflow_core::{
    ControlPortId, ExecutionEvent, ExecutionEventKind, ExecutionEventPayload, FlowScopeBoundary,
    WorkflowEdge, WorkflowNode,
};

use super::{
    engine::{ExecutionEventSink, emit_traced_event},
    execution_events::build_event,
    loop_runtime::{LoopFrame, append_loop_path},
    path_node_executor::{execute_node, publish_node_result, record_node_started},
    run_context::RunContext,
    scheduler::ResourceScheduler,
};
use crate::{
    NodeExecution, RuntimeError, node_registry::NodeFlow, validation::validator::PreparedWorkflow,
};

/// 当前待执行的作用域和节点。
#[derive(Debug)]
struct Cursor {
    /// 节点所属作用域。
    scope_id: String,
    /// 全局唯一节点 ID。
    node_id: String,
}

/// 执行期间不再变化的节点、边和作用域索引。
struct PathIndex<'workflow> {
    /// 节点 ID 到所属作用域和定义。
    nodes: HashMap<&'workflow str, (&'workflow str, &'workflow WorkflowNode)>,
    /// 节点 ID 到文档顺序出边。
    outgoing: HashMap<&'workflow str, Vec<&'workflow WorkflowEdge>>,
    /// 作用域 ID 到固定入口节点 ID。
    entries: HashMap<&'workflow str, &'workflow str>,
}

/// 单节点执行共享的只读运行设施。
#[derive(Clone, Copy)]
pub(super) struct NodeExecutionEnvironment<'runtime> {
    /// 当前已经完成校验与冻结的工作流。
    pub(super) workflow: &'runtime PreparedWorkflow,
    /// 实时事件接收器。
    pub(super) sink: &'runtime Arc<dyn ExecutionEventSink>,
    /// 可选持久化 Trace 会话。
    pub(super) trace: Option<&'runtime Arc<dyn crate::RunTraceSession>>,
    /// 跨 RunWorld 资源调度器。
    pub(super) scheduler: &'runtime ResourceScheduler,
}

/// 沿唯一命中路径执行节点；While 重复只改变显式帧栈，不递归调用执行器。
pub(super) async fn execute_path(
    workflow: &PreparedWorkflow,
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    context: &mut RunContext,
    sequence: &mut u64,
    scheduler: &ResourceScheduler,
) -> Result<(), RuntimeError> {
    let index = PathIndex::new(workflow)?;
    let root_scope_id = workflow.definition.graph.root_scope_id.clone();
    let root_entry = index
        .entries
        .get(root_scope_id.as_str())
        .ok_or_else(|| RuntimeError::ExecutionInvariant("根作用域入口在校验后消失".to_owned()))?;
    let mut current = Some(Cursor {
        scope_id: root_scope_id,
        node_id: (*root_entry).to_owned(),
    });
    let mut loop_stack = Vec::<LoopFrame>::new();
    let environment = NodeExecutionEnvironment {
        workflow,
        sink,
        trace,
        scheduler,
    };

    while let Some(cursor) = current {
        let (scope_id, node) = index
            .nodes
            .get(cursor.node_id.as_str())
            .copied()
            .ok_or_else(|| {
                RuntimeError::ExecutionInvariant(format!("节点 '{}' 在校验后消失", cursor.node_id))
            })?;
        if scope_id != cursor.scope_id {
            return Err(RuntimeError::ExecutionInvariant(format!(
                "节点 '{}' 不属于调度作用域 '{}'",
                cursor.node_id, cursor.scope_id,
            )));
        }
        let prepared = workflow.nodes.get(&node.id).ok_or_else(|| {
            RuntimeError::ExecutionInvariant(format!("节点 '{}' 没有冻结执行对象", node.id))
        })?;
        let flow = prepared.flow();
        if let NodeFlow::Loop {
            body_scope_id,
            max_iterations,
            timeout_ms,
            interval_ms,
            ..
        } = flow
        {
            record_node_started(environment, context, sequence, node, prepared, &loop_stack)?;
            emit_path_event(
                environment,
                build_event(
                    context.run_id,
                    workflow.definition.id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::LoopStarted,
                    Some("开始执行 While 容器".to_owned()),
                    Some(ExecutionEventPayload::LoopStarted {
                        scope_id: body_scope_id.clone(),
                        max_iterations,
                    }),
                ),
                &loop_stack,
            )?;
            loop_stack.push(LoopFrame::new(
                cursor.scope_id,
                node.id.clone(),
                body_scope_id,
                max_iterations,
                timeout_ms,
                interval_ms,
            ));
            current = begin_next_iteration(environment, context, sequence, &index, &mut loop_stack)
                .await?;
            continue;
        }

        record_node_started(environment, context, sequence, node, prepared, &loop_stack)?;
        let execution =
            execute_node(environment, context, sequence, node, prepared, &loop_stack).await?;
        let selected_branch = execution.branch.clone();
        let termination = execution.termination.clone();
        publish_node_result(environment, context, sequence, node, execution, &loop_stack)?;
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

        current = match flow {
            NodeFlow::LoopContinue => {
                verify_loop_boundary(&cursor, loop_stack.last(), "继续下一轮")?;
                match begin_next_iteration(environment, context, sequence, &index, &mut loop_stack)
                    .await?
                {
                    Some(next) => Some(next),
                    None => finish_loop(
                        environment,
                        context,
                        sequence,
                        &index,
                        &mut loop_stack,
                        "exhausted",
                    )?,
                }
            }
            NodeFlow::LoopComplete => {
                verify_loop_boundary(&cursor, loop_stack.last(), "完成循环")?;
                finish_loop(
                    environment,
                    context,
                    sequence,
                    &index,
                    &mut loop_stack,
                    "completed",
                )?
            }
            NodeFlow::End => None,
            NodeFlow::Start | NodeFlow::Linear | NodeFlow::Branch { .. } | NodeFlow::LoopEntry => {
                traverse_selected_edge(
                    environment,
                    context,
                    sequence,
                    &index,
                    &loop_stack,
                    &cursor.scope_id,
                    &node.id,
                    selected_branch.as_ref(),
                )?
            }
            NodeFlow::Loop { .. } => unreachable!("While 已在上方处理"),
        };
    }
    if loop_stack.is_empty() {
        Ok(())
    } else {
        Err(RuntimeError::ExecutionInvariant(
            "执行路径在 While 子作用域中意外结束".to_owned(),
        ))
    }
}

impl<'workflow> PathIndex<'workflow> {
    /// 从已校验定义建立全局只读索引。
    fn new(workflow: &'workflow PreparedWorkflow) -> Result<Self, RuntimeError> {
        let mut nodes = HashMap::new();
        let mut outgoing = HashMap::<&str, Vec<&WorkflowEdge>>::new();
        let mut entries = HashMap::new();
        for scope in &workflow.definition.graph.scopes {
            let entry = match &scope.boundary {
                FlowScopeBoundary::Workflow { entry_node_id }
                | FlowScopeBoundary::Component { entry_node_id, .. }
                | FlowScopeBoundary::Loop { entry_node_id, .. } => entry_node_id,
            };
            entries.insert(scope.id.as_str(), entry.as_str());
            for node in &scope.nodes {
                nodes.insert(node.id.as_str(), (scope.id.as_str(), node));
            }
            for edge in &scope.edges {
                outgoing.entry(edge.source.as_str()).or_default().push(edge);
            }
        }
        if nodes.is_empty() {
            return Err(RuntimeError::ExecutionInvariant(
                "工作流没有可执行节点".to_owned(),
            ));
        }
        Ok(Self {
            nodes,
            outgoing,
            entries,
        })
    }
}

/// 进入活动 While 的下一轮，或在预算耗尽时通知调用方完成 exhausted 退出。
async fn begin_next_iteration(
    environment: NodeExecutionEnvironment<'_>,
    context: &RunContext,
    sequence: &mut u64,
    index: &PathIndex<'_>,
    loop_stack: &mut [LoopFrame],
) -> Result<Option<Cursor>, RuntimeError> {
    let frame = loop_stack
        .last_mut()
        .ok_or_else(|| RuntimeError::ExecutionInvariant("缺少活动 While 帧".to_owned()))?;
    let Some(iteration) = frame.begin_next_iteration().await else {
        return Ok(None);
    };
    let entry = index
        .entries
        .get(frame.body_scope_id.as_str())
        .ok_or_else(|| {
            RuntimeError::ExecutionInvariant(format!(
                "While 子作用域 '{}' 没有入口",
                frame.body_scope_id
            ))
        })?;
    let node_id = frame.container_node_id.clone();
    let scope_id = frame.body_scope_id.clone();
    let max_iterations = frame.max_iterations;
    emit_path_event(
        environment,
        build_event(
            context.run_id,
            environment.workflow.definition.id,
            sequence,
            Some(node_id),
            None,
            ExecutionEventKind::LoopIteration,
            Some(format!("开始第 {iteration} 次重复")),
            Some(ExecutionEventPayload::LoopIteration {
                iteration,
                max_iterations,
            }),
        ),
        loop_stack,
    )?;
    Ok(Some(Cursor {
        scope_id,
        node_id: (*entry).to_owned(),
    }))
}

/// 以 completed 或 exhausted 端口结束当前 While，并回到父作用域。
fn finish_loop(
    environment: NodeExecutionEnvironment<'_>,
    context: &mut RunContext,
    sequence: &mut u64,
    index: &PathIndex<'_>,
    loop_stack: &mut Vec<LoopFrame>,
    branch: &str,
) -> Result<Option<Cursor>, RuntimeError> {
    let frame = loop_stack
        .last()
        .ok_or_else(|| RuntimeError::ExecutionInvariant("结束 While 时缺少活动帧".to_owned()))?;
    let event_kind = if branch == "completed" {
        ExecutionEventKind::LoopCompleted
    } else {
        ExecutionEventKind::LoopExhausted
    };
    let payload = if branch == "completed" {
        ExecutionEventPayload::LoopCompleted {
            iterations: frame.iterations(),
        }
    } else {
        ExecutionEventPayload::LoopExhausted {
            iterations: frame.iterations(),
        }
    };
    emit_path_event(
        environment,
        build_event(
            context.run_id,
            environment.workflow.definition.id,
            sequence,
            Some(frame.container_node_id.clone()),
            None,
            event_kind,
            Some(if branch == "completed" {
                "While 已正常完成".to_owned()
            } else {
                "While 已达到次数或时间上限".to_owned()
            }),
            Some(payload),
        ),
        loop_stack,
    )?;
    let frame = loop_stack.pop().expect("上方已证明活动帧存在");
    let (_, container) = index
        .nodes
        .get(frame.container_node_id.as_str())
        .copied()
        .ok_or_else(|| RuntimeError::ExecutionInvariant("While 容器在退出时消失".to_owned()))?;
    publish_node_result(
        environment,
        context,
        sequence,
        container,
        NodeExecution::default(),
        loop_stack,
    )?;
    traverse_selected_edge(
        environment,
        context,
        sequence,
        index,
        loop_stack,
        &frame.parent_scope_id,
        &frame.container_node_id,
        Some(&ControlPortId::new(branch)),
    )
}

/// 确保 Continue/Complete 只能结束当前栈顶 While 子作用域。
fn verify_loop_boundary(
    cursor: &Cursor,
    frame: Option<&LoopFrame>,
    label: &str,
) -> Result<(), RuntimeError> {
    if frame.is_some_and(|frame| frame.body_scope_id == cursor.scope_id) {
        Ok(())
    } else {
        Err(RuntimeError::ExecutionInvariant(format!(
            "{label}边界不属于当前 While 帧"
        )))
    }
}

/// 根据分支值选择唯一后继边并发出遍历事件。
#[allow(clippy::too_many_arguments)]
fn traverse_selected_edge(
    environment: NodeExecutionEnvironment<'_>,
    context: &RunContext,
    sequence: &mut u64,
    index: &PathIndex<'_>,
    loop_stack: &[LoopFrame],
    scope_id: &str,
    node_id: &str,
    selected_branch: Option<&ControlPortId>,
) -> Result<Option<Cursor>, RuntimeError> {
    let edge = index
        .outgoing
        .get(node_id)
        .into_iter()
        .flatten()
        .find(|edge| edge.branch.as_ref() == selected_branch);
    let Some(edge) = edge else { return Ok(None) };
    emit_path_event(
        environment,
        build_event(
            context.run_id,
            environment.workflow.definition.id,
            sequence,
            None,
            Some(edge.id.clone()),
            ExecutionEventKind::EdgeTraversed,
            Some(format!("{} → {}", edge.source, edge.target)),
            None,
        ),
        loop_stack,
    )?;
    Ok(Some(Cursor {
        scope_id: scope_id.to_owned(),
        node_id: edge.target.clone(),
    }))
}

/// 附加 While 结构路径后写入 Trace 和实时事件流。
pub(super) fn emit_path_event(
    environment: NodeExecutionEnvironment<'_>,
    mut event: ExecutionEvent,
    loop_stack: &[LoopFrame],
) -> Result<(), RuntimeError> {
    append_loop_path(&mut event, loop_stack);
    emit_traced_event(
        environment.sink,
        environment.trace,
        event,
        &environment.workflow.source_map,
    )
}
