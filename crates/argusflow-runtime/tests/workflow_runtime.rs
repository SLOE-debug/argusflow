//! 工作流执行事件、运行输入、分支选择与 RunWorld 并发契约。

mod workflow_fixture;

use std::sync::Arc;

use argusflow_core::{
    AqlQuery, AutomationTarget, ControlPortId, ExecutionEvent, ExecutionEventKind, RunInputs,
    UiOperation, ValueExpr, ValueSource, WorkflowEdge, WorkflowInputDefinition, WorkflowInputType,
};
use argusflow_runtime::{
    ExecutionEventSink, FileRunTraceStore, RunStatus, RunTraceLevel, RuntimeError,
    UnavailableActionDispatcher, WorkflowEngine,
};
use serde_json::json;
use tokio::sync::mpsc;

use workflow_fixture::{WorkflowNodeKind, condition_workflow, demo_workflow, edge};

/// 将运行时事件转发到测试接收端的内存 sink。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    /// 将事件交给无界通道；通道关闭时把发送错误转换成 sink 错误。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

#[tokio::test]
async fn runtime_requires_and_resolves_separate_run_inputs() {
    let mut workflow = demo_workflow(1);
    workflow.inputs = vec![WorkflowInputDefinition {
        key: "secret".to_owned(),
        value_type: WorkflowInputType::Text,
    }];
    workflow.nodes[1].definition = WorkflowNodeKind::Debug {
        value: ValueExpr::Ref {
            source: ValueSource::WorkflowInput {
                key: "secret".to_owned(),
            },
            pointer: String::new(),
        },
    }
    .into();
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();

    let missing_error = engine
        .start(
            workflow.clone(),
            RunInputs::default(),
            Arc::new(ChannelSink(sender.clone())),
        )
        .await
        .expect_err("a required run input must be provided");
    assert!(matches!(
        missing_error,
        RuntimeError::InvalidRunInputs { .. }
    ));

    let input_values = json!({ "secret": "ephemeral" })
        .as_object()
        .expect("fixture run inputs should be an object")
        .clone();
    engine
        .start(
            workflow,
            RunInputs {
                values: input_values,
            },
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("declared run inputs should be accepted");

    let mut observed = false;
    while let Some(event) = receiver.recv().await {
        observed |=
            event.kind == ExecutionEventKind::Log && event.message.as_deref() == Some("ephemeral");
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }
    assert!(observed);
}

#[tokio::test]
async fn runtime_persists_a_completed_run_without_plaintext_inputs() {
    let run_root =
        std::env::temp_dir().join(format!("argusflow-run-trace-{}", uuid::Uuid::new_v4()));
    let trace_store = Arc::new(FileRunTraceStore::new(
        &run_root,
        RunTraceLevel::Diagnostics,
    ));
    let engine = Arc::new(
        WorkflowEngine::new(Arc::new(UnavailableActionDispatcher))
            .with_trace_store(trace_store.clone()),
    );
    let mut workflow = demo_workflow(1);
    workflow.inputs = vec![WorkflowInputDefinition {
        key: "secret".to_owned(),
        value_type: WorkflowInputType::Text,
    }];
    workflow.nodes[1].definition = WorkflowNodeKind::Debug {
        value: ValueExpr::Ref {
            source: ValueSource::WorkflowInput {
                key: "secret".to_owned(),
            },
            pointer: String::new(),
        },
    }
    .into();
    let values = json!({ "secret": "must-not-be-persisted" })
        .as_object()
        .expect("fixture input should be an object")
        .clone();
    let (sender, mut receiver) = mpsc::unbounded_channel();

    let started = engine
        .start(
            workflow,
            RunInputs { values },
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("trace-enabled run should start");
    while let Some(event) = receiver.recv().await {
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }
    // Engine 在发出终态事件后立即 finalize Manifest；等待活动集合完成移除以避免竞态断言。
    while engine.active_runs().await.contains(&started.run_id) {
        tokio::task::yield_now().await;
    }

    let details = trace_store
        .get_run(started.run_id)
        .expect("completed run should be readable");
    assert_eq!(details.manifest.status, RunStatus::Completed);
    let events = trace_store
        .read_events(started.run_id)
        .expect("trace JSONL should be readable");
    assert!(
        events
            .iter()
            .all(|event| { event.event.message.as_deref() != Some("must-not-be-persisted") })
    );
    let run_inputs = std::fs::read_to_string(
        run_root
            .join(started.run_id.to_string())
            .join("workflow/run-inputs.json"),
    )
    .expect("redacted run inputs should exist");
    assert!(!run_inputs.contains("must-not-be-persisted"));
}

#[tokio::test]
async fn runtime_emits_ordered_log_and_completion_events() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    engine
        .start(
            demo_workflow(1),
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
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
        events
            .iter()
            .filter(|event| event.kind == ExecutionEventKind::EdgeTraversed)
            .count(),
        3
    );
    assert_eq!(
        events.last().map(|event| event.kind),
        Some(ExecutionEventKind::WorkflowCompleted)
    );
}

#[tokio::test]
async fn runtime_only_executes_the_selected_condition_branch() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    engine
        .start(
            condition_workflow(false),
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("run should start");
    let mut edge_ids = Vec::new();
    while let Some(event) = receiver.recv().await {
        if event.kind == ExecutionEventKind::EdgeTraversed {
            edge_ids.push(
                event
                    .edge_id
                    .clone()
                    .expect("edge event should carry its id"),
            );
        }
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }
    assert!(edge_ids.contains(&"condition-false".to_owned()));
    assert!(!edge_ids.contains(&"condition-true".to_owned()));
}

#[tokio::test]
async fn condition_reads_variables_committed_by_a_prior_variable_node() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workflow = condition_workflow(false);
    workflow.nodes.insert(
        1,
        workflow_fixture::node(
            "set-enabled",
            80.0,
            WorkflowNodeKind::SetVariable {
                name: "enabled".to_owned(),
                value: ValueExpr::Literal { value: json!(true) },
            },
        ),
    );
    workflow.edges.retain(|edge| edge.id != "start-condition");
    workflow
        .edges
        .push(workflow_fixture::edge("start", "set-enabled"));
    workflow
        .edges
        .push(workflow_fixture::edge("set-enabled", "condition"));

    engine
        .start(
            workflow,
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("variable-driven condition should start");

    let mut selected_true = false;
    while let Some(event) = receiver.recv().await {
        selected_true |= event.edge_id.as_deref() == Some("condition-true");
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }
    assert!(selected_true);
}

#[tokio::test(start_paused = true)]
async fn runtime_accepts_multiple_active_run_worlds() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, _receiver) = mpsc::unbounded_channel();
    let sink = Arc::new(ChannelSink(sender));

    engine
        .start(demo_workflow(1_000), RunInputs::default(), sink.clone())
        .await
        .expect("first run should start");
    engine
        .start(demo_workflow(1_000), RunInputs::default(), sink)
        .await
        .expect("second run on independent state should start");

    assert_eq!(engine.active_runs().await.len(), 2);
}

#[tokio::test]
async fn bounded_loop_emits_each_iteration_then_exits_through_exhausted() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workflow = demo_workflow(1);
    workflow.nodes[2].definition = WorkflowNodeKind::Loop {
        max_iterations: 2,
        timeout_ms: 1_000,
        interval_ms: 0,
    }
    .into();
    workflow.edges = vec![
        edge("start", "delay"),
        WorkflowEdge {
            id: "loop-body".to_owned(),
            source: "delay".to_owned(),
            target: "log".to_owned(),
            branch: Some(ControlPortId::new("iterate")),
        },
        edge("log", "delay"),
        WorkflowEdge {
            id: "loop-exit".to_owned(),
            source: "delay".to_owned(),
            target: "end".to_owned(),
            branch: Some(ControlPortId::new("exhausted")),
        },
    ];

    engine
        .start(
            workflow,
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("bounded loop should start");
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        let completed = event.kind == ExecutionEventKind::WorkflowCompleted;
        events.push(event);
        if completed {
            break;
        }
    }

    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == ExecutionEventKind::LoopIteration)
            .count(),
        2,
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == ExecutionEventKind::LoopExhausted)
            .count(),
        1,
    );
}

#[tokio::test]
async fn fail_node_declares_a_typed_workflow_failure() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workflow = demo_workflow(1);
    workflow.nodes = vec![
        workflow.nodes.remove(0),
        workflow_fixture::node(
            "fail",
            220.0,
            WorkflowNodeKind::Fail {
                code: "state_rejected".to_owned(),
                message: ValueExpr::text("状态不符合业务约束"),
            },
        ),
    ];
    workflow.edges = vec![edge("start", "fail")];

    engine
        .start(
            workflow,
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("fail workflow should be accepted before execution");
    let mut declared = None;
    let mut failed = None;
    while let Some(event) = receiver.recv().await {
        if event.kind == ExecutionEventKind::WorkflowFailureDeclared {
            declared = event.payload.clone();
        }
        if event.kind == ExecutionEventKind::WorkflowFailed {
            failed = event.message;
            break;
        }
    }

    assert_eq!(
        declared,
        Some(
            argusflow_core::ExecutionEventPayload::WorkflowFailureDeclared {
                code: "state_rejected".to_owned(),
            }
        ),
    );
    assert!(failed.is_some_and(|message| message.contains("state_rejected")));
}

#[tokio::test]
async fn runtime_emits_a_failure_for_an_unavailable_action_backend() {
    let engine = Arc::new(WorkflowEngine::new(Arc::new(UnavailableActionDispatcher)));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = WorkflowNodeKind::Ui {
        operation: UiOperation::Click {
            target: AutomationTarget::query(AqlQuery::v3("button(name = \"保存\")")),
        },
    }
    .into();

    engine
        .start(
            workflow,
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
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
