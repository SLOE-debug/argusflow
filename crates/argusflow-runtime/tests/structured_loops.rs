//! 结构化 While 的任意嵌套与显式执行栈契约。

mod workflow_fixture;

use std::sync::Arc;

use argusflow_core::{
    ControlPortId, ExecutionEvent, ExecutionEventKind, ExecutionStructureFrame, FlowScope,
    FlowScopeBoundary, FlowScopeParent, NodeEnvelope, Position, RunInputs, Size, WorkflowEdge,
    WorkflowNode,
};
use argusflow_runtime::{
    ExecutionEventSink, UnavailableActionDispatcher, WorkflowEngine, validate_workflow,
};
use serde_json::json;
use tokio::sync::mpsc;

use workflow_fixture::{WorkflowNodeKind, edge, workflow_definition};

/// 将深层嵌套执行事件转发到测试线程。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

#[tokio::test]
async fn runtime_executes_512_nested_loops_without_recursive_graphs() {
    const DEPTH: usize = 512;

    let workflow = nested_loop_workflow(DEPTH);
    let report = validate_workflow(&workflow);
    assert!(report.valid, "{:#?}", report.issues);

    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    engine
        .start(
            workflow,
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("深层结构化循环应通过显式栈启动");

    let mut loop_started = 0usize;
    let mut loop_completed = 0usize;
    let mut deepest_path = 0usize;
    while let Some(event) = receiver.recv().await {
        loop_started += usize::from(event.kind == ExecutionEventKind::LoopStarted);
        loop_completed += usize::from(event.kind == ExecutionEventKind::LoopCompleted);
        deepest_path = deepest_path.max(
            event
                .structure_path
                .iter()
                .filter(|frame| matches!(frame, ExecutionStructureFrame::Loop { .. }))
                .count(),
        );
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }

    assert_eq!(loop_started, DEPTH);
    assert_eq!(loop_completed, DEPTH);
    assert_eq!(deepest_path, DEPTH);
}

/// 使用扁平 scope 表构造指定深度的嵌套 While，不让序列化深度随嵌套增长。
fn nested_loop_workflow(depth: usize) -> argusflow_core::WorkflowDefinition {
    assert!(depth > 0);
    let mut workflow = workflow_definition(
        "Deep structured loops",
        vec![
            plain_node("start", "argus.start"),
            loop_node("loop-0", "scope-0"),
            plain_node("end", "argus.end"),
        ],
        vec![
            edge("start", "loop-0"),
            branch_edge("root-completed", "loop-0", "end", "completed"),
            branch_edge("root-exhausted", "loop-0", "end", "exhausted"),
        ],
    );

    for index in 0..depth {
        workflow.graph.scopes.push(loop_scope(index, depth));
    }
    workflow
}

/// 构造一层循环正文；除最深层外，正文只包含下一层容器。
fn loop_scope(index: usize, depth: usize) -> FlowScope {
    let scope_id = format!("scope-{index}");
    let entry_id = format!("entry-{index}");
    let continue_id = format!("continue-{index}");
    let complete_id = format!("complete-{index}");
    let mut nodes = vec![
        plain_node(&entry_id, "argus.loop.entry"),
        plain_node(&continue_id, "argus.loop.continue"),
        plain_node(&complete_id, "argus.loop.complete"),
    ];
    let edges = if index + 1 == depth {
        vec![edge(&entry_id, &complete_id)]
    } else {
        let child_loop_id = format!("loop-{}", index + 1);
        nodes.push(loop_node(&child_loop_id, &format!("scope-{}", index + 1)));
        vec![
            edge(&entry_id, &child_loop_id),
            branch_edge(
                &format!("completed-{index}"),
                &child_loop_id,
                &complete_id,
                "completed",
            ),
            branch_edge(
                &format!("exhausted-{index}"),
                &child_loop_id,
                &complete_id,
                "exhausted",
            ),
        ]
    };
    FlowScope {
        id: scope_id,
        parent: Some(FlowScopeParent {
            scope_id: if index == 0 {
                "root".to_owned()
            } else {
                format!("scope-{}", index - 1)
            },
            node_id: format!("loop-{index}"),
        }),
        boundary: FlowScopeBoundary::Loop {
            entry_node_id: entry_id,
            continue_node_id: continue_id,
            complete_node_id: complete_id,
        },
        nodes,
        edges,
    }
}

/// 构造持有独立正文作用域的 While 容器。
fn loop_node(id: &str, body_scope_id: &str) -> WorkflowNode {
    let mut node = workflow_fixture::node(
        id,
        0.0,
        WorkflowNodeKind::Loop {
            body_scope_id: body_scope_id.to_owned(),
            max_iterations: 1,
            timeout_ms: 60_000,
            interval_ms: 0,
        },
    );
    node.size = Size {
        width: 420.0,
        height: 240.0,
    };
    node
}

/// 构造无 payload 的结构边界节点。
fn plain_node(id: &str, type_id: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x: 0.0, y: 0.0 },
        size: Size {
            width: 142.0,
            height: 52.0,
        },
        definition: NodeEnvelope::new(type_id, 1, json!({})),
        output_bindings: Default::default(),
    }
}

/// 构造结构容器的一个具名出口。
fn branch_edge(id: &str, source: &str, target: &str, branch: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: id.to_owned(),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: Some(ControlPortId::new(branch)),
    }
}
