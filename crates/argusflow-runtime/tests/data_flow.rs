//! ValueExpr 驱动的 UI Read → SetValue 运行时数据流验收。

use std::{collections::BTreeMap, sync::Arc};

use argusflow_core::{
    ActionOutcome, AqlQuery, AutomationAction, AutomationError, AutomationExecutionScope,
    AutomationTarget, BackendKind, DiagnosticEvidenceReference, ExecutionEvent, ExecutionEventKind,
    ExecutionEventPayload, NodeEnvelope, Position, RunInputs, UiOperation, ValueExpr,
    WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::{ActionDispatcher, ExecutionEventSink, WorkflowEngine};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

/// 测试 fixture 使用的内置节点构造器。
enum WorkflowNodeKind {
    Start,
    Ui { operation: UiOperation },
    Debug { value: ValueExpr },
    End,
}

impl From<WorkflowNodeKind> for NodeEnvelope {
    fn from(kind: WorkflowNodeKind) -> Self {
        match kind {
            WorkflowNodeKind::Start => Self::new("argus.start", 1, json!({})),
            WorkflowNodeKind::Ui { operation } => {
                Self::new("argus.ui", 1, json!({ "operation": operation }))
            }
            WorkflowNodeKind::Debug { value } => {
                Self::new("argus.debug", 1, json!({ "value": value }))
            }
            WorkflowNodeKind::End => Self::new("argus.end", 1, json!({})),
        }
    }
}

/// 记录 Runtime 交付的已解析动作，并为读取动作返回固定文本端口。
#[derive(Default)]
struct CapturingDispatcher {
    /// 按真实执行顺序保存动作，供测试验证 SetValue 不再携带 ValueExpr。
    actions: Mutex<Vec<AutomationAction>>,
}

#[async_trait]
impl ActionDispatcher for CapturingDispatcher {
    async fn execute(
        &self,
        action: &AutomationAction,
        _scope: AutomationExecutionScope,
    ) -> Result<ActionOutcome, AutomationError> {
        self.actions.lock().await.push(action.clone());
        let outputs = match action {
            AutomationAction::GetText { .. } => {
                BTreeMap::from([("text".to_owned(), Value::String("ACME-10086".to_owned()))])
            }
            AutomationAction::Click { .. }
            | AutomationAction::SetValue { .. }
            | AutomationAction::GetValue { .. } => BTreeMap::new(),
            AutomationAction::CollectLinks { .. } => BTreeMap::from([
                ("text".to_owned(), Value::String(String::new())),
                ("links".to_owned(), Value::Array(Vec::new())),
            ]),
        };
        Ok(ActionOutcome {
            backend: BackendKind::WindowsUia,
            message: "captured".to_owned(),
            outputs,
            diagnostic_evidence: vec![DiagnosticEvidenceReference {
                evidence_id: Uuid::nil(),
                backend: BackendKind::WindowsUia,
                branch_path: vec![0],
                recovered_by_fallback: true,
            }],
        })
    }
}

/// 将事件转发给测试任务，避免依赖 Tauri 事件桥接。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

#[tokio::test]
async fn read_output_is_resolved_for_debug_and_the_following_set_value() {
    let dispatcher = Arc::new(CapturingDispatcher::default());
    let engine = Arc::new(WorkflowEngine::new(dispatcher.clone()));
    let (sender, mut receiver) = mpsc::unbounded_channel();

    engine
        .start(
            read_then_write_workflow(),
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("data-flow workflow should start");

    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        let completed = event.kind == ExecutionEventKind::WorkflowCompleted;
        events.push(event);
        if completed {
            break;
        }
    }

    let actions = dispatcher.actions.lock().await;
    assert!(matches!(
        actions.first(),
        Some(AutomationAction::GetText { .. })
    ));
    assert!(matches!(
        actions.get(1),
        Some(AutomationAction::SetValue { value, .. }) if value == "ACME-10086"
    ));
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::NodeOutputProduced
            && event.payload
                == Some(ExecutionEventPayload::NodeOutputsProduced {
                    output_names: vec!["text".to_owned()],
                })
    }));
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::DiagnosticEvidenceCaptured
            && event.payload
                == Some(ExecutionEventPayload::DiagnosticEvidenceCaptured {
                    evidence_id: Uuid::nil(),
                    backend: BackendKind::WindowsUia,
                    branch_path: vec![0],
                    recovered_by_fallback: true,
                })
    }));
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::Log
            && event.node_id.as_deref() == Some("debug")
            && event.message.as_deref() == Some("ACME-10086")
    }));
}

/// 构造 Start → GetText → Debug/SetValue(NodeOutput) → End 的最小数据流。
fn read_then_write_workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 8,
        id: Uuid::new_v4(),
        name: "Read then write".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: WorkflowPermissions::default(),
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "read",
                180.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::GetText {
                        target: AutomationTarget::query(AqlQuery::v1(
                            "first(textbox(name = \"订单号\"))",
                        )),
                    },
                },
            ),
            node(
                "debug",
                360.0,
                WorkflowNodeKind::Debug {
                    value: ValueExpr::node("read", "/text"),
                },
            ),
            node(
                "write",
                540.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::SetValue {
                        target: AutomationTarget::query(AqlQuery::v1(
                            "first(textbox(name = \"订单编号\"))",
                        )),
                        value: ValueExpr::node("read", "/text"),
                    },
                },
            ),
            node("end", 720.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "read"),
            edge("read", "debug"),
            edge("debug", "write"),
            edge("write", "end"),
        ],
    }
}

/// 创建固定纵坐标的测试节点。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        definition: kind.into(),
        output_bindings: Default::default(),
    }
}

/// 创建线性测试边。
fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}
