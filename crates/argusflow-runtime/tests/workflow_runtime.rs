//! 工作流运行时的结构校验、事件顺序和失败传播测试。
//!
//! 测试 fixture 构造线性工作流，并通过内存通道观察异步引擎发出的生命周期事件。

use std::sync::Arc;

use argusflow_core::{
    AutomationAction, ExecutionEvent, ExecutionEventKind, Position, Selector, WorkflowDefinition,
    WorkflowEdge, WorkflowNode, WorkflowNodeKind,
};
use argusflow_runtime::{
    ExecutionEventSink, RuntimeError, UnavailableActionDispatcher, ValidationIssueCode,
    WorkflowEngine, validate_workflow,
};
use tokio::sync::mpsc;
use uuid::Uuid;

/// 将运行时事件转发到测试接收端的内存 sink。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    /// 将事件交给无界通道；通道关闭时把发送错误转换成 sink 错误。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

#[test]
fn valid_linear_workflow_passes_validation() {
    assert!(validate_workflow(&demo_workflow(1)).valid);
}

#[test]
fn validation_rejects_duplicate_ids_unknown_edges_cycles_branches_and_unreachable_nodes() {
    let mut duplicate = demo_workflow(1);
    duplicate.nodes[1].id = "start".to_owned();
    assert_has_issue(&duplicate, ValidationIssueCode::DuplicateNodeId);

    let mut unknown_edge = demo_workflow(1);
    unknown_edge.edges[0].target = "missing".to_owned();
    assert_has_issue(&unknown_edge, ValidationIssueCode::UnknownEdgeEndpoint);

    let mut duplicate_edge = demo_workflow(1);
    duplicate_edge.edges[1].id = duplicate_edge.edges[0].id.clone();
    assert_has_issue(&duplicate_edge, ValidationIssueCode::DuplicateEdgeId);

    let mut cycle = demo_workflow(1);
    cycle.edges.push(WorkflowEdge {
        id: "cycle".to_owned(),
        source: "end".to_owned(),
        target: "start".to_owned(),
    });
    assert_has_issue(&cycle, ValidationIssueCode::CycleDetected);

    let mut branch = demo_workflow(1);
    branch.nodes.insert(
        2,
        WorkflowNode {
            id: "extra".to_owned(),
            position: Position { x: 400.0, y: 120.0 },
            kind: WorkflowNodeKind::Log {
                message: "branch".to_owned(),
            },
        },
    );
    branch.edges.push(WorkflowEdge {
        id: "branch".to_owned(),
        source: "start".to_owned(),
        target: "extra".to_owned(),
    });
    assert_has_issue(&branch, ValidationIssueCode::InvalidNodeDegree);

    let mut unreachable = demo_workflow(1);
    unreachable.nodes.push(WorkflowNode {
        id: "orphan".to_owned(),
        position: Position { x: 0.0, y: 200.0 },
        kind: WorkflowNodeKind::Log {
            message: "orphan".to_owned(),
        },
    });
    assert_has_issue(&unreachable, ValidationIssueCode::UnreachableNode);
}

#[test]
fn validation_requires_exactly_one_start_and_end() {
    let mut workflow = demo_workflow(1);
    workflow
        .nodes
        .retain(|node| !matches!(&node.kind, WorkflowNodeKind::End));
    assert_has_issue(&workflow, ValidationIssueCode::InvalidEndCount);

    let mut workflow = demo_workflow(1);
    workflow.nodes.push(WorkflowNode {
        id: "another-start".to_owned(),
        position: Position { x: 0.0, y: 100.0 },
        kind: WorkflowNodeKind::Start,
    });
    assert_has_issue(&workflow, ValidationIssueCode::InvalidStartCount);
}

#[tokio::test]
async fn runtime_emits_ordered_log_and_completion_events() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    engine
        .start(demo_workflow(1), Arc::new(ChannelSink(sender)))
        .await
        .expect("run should start");

    // 只收集到 WorkflowCompleted，避免异步任务结束后的通道关闭成为测试前提。
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        let completed = event.kind == ExecutionEventKind::WorkflowCompleted;
        events.push(event);
        if completed {
            break;
        }
    }

    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::Log && event.message.as_deref() == Some("ArgusFlow")
    }));
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(ExecutionEventKind::WorkflowCompleted)
    );
}

#[tokio::test(start_paused = true)]
async fn runtime_rejects_a_second_active_run() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, _receiver) = mpsc::unbounded_channel();
    let sink = Arc::new(ChannelSink(sender));

    engine
        .start(demo_workflow(1_000), sink.clone())
        .await
        .expect("first run should start");
    let error = engine
        .start(demo_workflow(1_000), sink)
        .await
        .expect_err("second run should be rejected");

    assert!(matches!(error, RuntimeError::RunInProgress { .. }));
}

#[tokio::test]
async fn runtime_emits_a_failure_for_an_unavailable_action_backend() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].kind = WorkflowNodeKind::Action {
        action: AutomationAction::Click {
            target: Selector::Native {
                name: Some("保存".to_owned()),
                automation_id: None,
                control_type: Some("Button".to_owned()),
            },
        },
    };

    engine
        .start(workflow, Arc::new(ChannelSink(sender)))
        .await
        .expect("run should be accepted before asynchronous execution");

    // 失败事件应在节点后端不可用时产生；无需等待任务返回值，因为 start 是异步接受执行。
    let mut failed = None;
    while let Some(event) = receiver.recv().await {
        if event.kind == ExecutionEventKind::WorkflowFailed {
            failed = Some(event);
            break;
        }
    }

    assert!(
        failed
            .and_then(|event| event.message)
            .is_some_and(|message| message.contains("unavailable"))
    );
}

fn assert_has_issue(workflow: &WorkflowDefinition, code: ValidationIssueCode) {
    let report = validate_workflow(workflow);
    assert!(report.issues.iter().any(|issue| issue.code == code));
}

/// 在测试中构造一条可执行的 Start -> Log -> Delay -> End 线性链。
fn demo_workflow(milliseconds: u64) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 1,
        id: Uuid::new_v4(),
        name: "Demo".to_owned(),
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "log",
                220.0,
                WorkflowNodeKind::Log {
                    message: "ArgusFlow".to_owned(),
                },
            ),
            node("delay", 440.0, WorkflowNodeKind::Delay { milliseconds }),
            node("end", 660.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "log"),
            edge("log", "delay"),
            edge("delay", "end"),
        ],
    }
}

/// 使用给定横坐标创建测试节点，统一 fixture 的画布布局。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        kind,
    }
}

/// 创建由源节点指向目标节点的测试连线，并派生稳定的连线 ID。
fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
    }
}
