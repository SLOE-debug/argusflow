//! Observe 事实输出驱动 Action 输入的运行时数据流验收。

use std::{collections::BTreeMap, sync::Arc};

use argusflow_core::{
    ActionExecutionOptions, ActionOutcome, AqlQuery, AutomationAction, AutomationError,
    AutomationExecutionScope, AutomationTarget, BackendKind, ControlPortId,
    DiagnosticEvidenceReference, ExecutionEvent, ExecutionEventKind, ExecutionEventPayload,
    NodeEnvelope, ObservationExecutionOptions, ObservationPolicy, ObservationRequest,
    ObservationResult, ObservationValue, ObserveSpec, Position, RunInputs, TargetScope,
    TargetWaitPolicy, UiExecutionPolicy, UiOperation, ValueExpr, WorkflowDefinition, WorkflowEdge,
    WorkflowNode,
};
use argusflow_runtime::{
    ActionDispatcher, ExecutionEventSink, ObservationDispatcher,
    UnavailableApplicationSessionProvider, UnavailableBrowserSessionProvider, WorkflowEngine,
};
use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;
mod workflow_fixture;
use workflow_fixture::workflow_definition;

/// 测试 fixture 使用的内置节点构造器。
enum WorkflowNodeKind {
    Start,
    Observe { observation: ObserveSpec },
    Ui { operation: UiOperation },
    Debug { value: ValueExpr },
    Fail,
    End,
}

impl From<WorkflowNodeKind> for NodeEnvelope {
    fn from(kind: WorkflowNodeKind) -> Self {
        match kind {
            WorkflowNodeKind::Start => Self::new("argus.start", 1, json!({})),
            WorkflowNodeKind::Observe { observation } => {
                Self::new("argus.observe", 1, json!({ "observation": observation }))
            }
            WorkflowNodeKind::Ui { operation } => Self::new(
                "argus.ui",
                5,
                json!({
                    "operation": operation,
                    "execution": UiExecutionPolicy {
                        target_wait: TargetWaitPolicy::bounded(5_000, 100),
                    },
                }),
            ),
            WorkflowNodeKind::Debug { value } => {
                Self::new("argus.debug", 1, json!({ "value": value }))
            }
            WorkflowNodeKind::Fail => Self::new(
                "argus.fail",
                1,
                json!({
                    "code": "observation_unknown",
                    "message": ValueExpr::text("观察结果未知"),
                }),
            ),
            WorkflowNodeKind::End => Self::new("argus.end", 1, json!({})),
        }
    }
}

/// 记录 Runtime 交付的已解析动作；Action 不再承担事实读取。
#[derive(Default)]
struct CapturingDispatcher {
    /// 按真实执行顺序保存动作，供测试验证 SetValue 不再携带 ValueExpr。
    actions: Mutex<Vec<AutomationAction>>,
    /// Runtime 从 UI payload 解码并传入的节点执行策略。
    options: Mutex<Vec<ActionExecutionOptions>>,
}

#[async_trait]
impl ActionDispatcher for CapturingDispatcher {
    async fn execute(
        &self,
        action: &AutomationAction,
        _scope: AutomationExecutionScope,
    ) -> Result<ActionOutcome, AutomationError> {
        self.actions.lock().await.push(action.clone());
        match action {
            AutomationAction::Click { .. }
            | AutomationAction::SetValue { .. }
            | AutomationAction::PressKey { .. }
            | AutomationAction::TypeText { .. } => {}
        }
        Ok(ActionOutcome {
            backend: BackendKind::WindowsUia,
            message: "captured".to_owned(),
            outputs: BTreeMap::new(),
            diagnostic_evidence: vec![DiagnosticEvidenceReference {
                evidence_id: Uuid::nil(),
                backend: BackendKind::WindowsUia,
                branch_path: vec![0],
                recovered_by_fallback: true,
            }],
        })
    }

    async fn execute_with_options(
        &self,
        action: &AutomationAction,
        scope: AutomationExecutionScope,
        options: ActionExecutionOptions,
    ) -> Result<ActionOutcome, AutomationError> {
        self.options.lock().await.push(options);
        self.execute(action, scope).await
    }
}

/// 为 Observe 返回固定的完整记录事实，并保留冻结请求供断言。
#[derive(Default)]
struct CapturingObservationDispatcher {
    requests: Mutex<Vec<ObservationRequest>>,
    /// Observe 节点交付的运行关联身份。
    options: Mutex<Vec<ObservationExecutionOptions>>,
}

#[async_trait]
impl ObservationDispatcher for CapturingObservationDispatcher {
    async fn observe(
        &self,
        request: &ObservationRequest,
        _scope: AutomationExecutionScope,
    ) -> ObservationResult {
        self.requests.lock().await.push(request.clone());
        ObservationResult::Known {
            backend: BackendKind::WindowsUia,
            value: ObservationValue::Records(vec![BTreeMap::from([(
                "text".to_owned(),
                Value::String("ACME-10086".to_owned()),
            )])]),
        }
    }

    async fn observe_with_options(
        &self,
        request: &ObservationRequest,
        scope: AutomationExecutionScope,
        options: ObservationExecutionOptions,
    ) -> ObservationResult {
        self.options.lock().await.push(options);
        self.observe(request, scope).await
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
async fn observation_output_is_resolved_for_debug_and_the_following_set_value() {
    let actions = Arc::new(CapturingDispatcher::default());
    let observations = Arc::new(CapturingObservationDispatcher::default());
    let engine = Arc::new(WorkflowEngine::with_dispatchers(
        actions.clone(),
        observations.clone(),
        Arc::new(UnavailableApplicationSessionProvider),
        Arc::new(UnavailableBrowserSessionProvider),
    ));
    let (sender, mut receiver) = mpsc::unbounded_channel();

    engine
        .start(
            observe_then_write_workflow(),
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

    let captured_actions = actions.actions.lock().await;
    assert!(matches!(
        captured_actions.first(),
        Some(AutomationAction::SetValue { value, .. }) if value == "ACME-10086"
    ));
    drop(captured_actions);
    assert_eq!(observations.requests.lock().await.len(), 1);
    let observation_options = observations.options.lock().await;
    assert_eq!(observation_options.len(), 1);
    let trace_context = observation_options[0]
        .trace_context
        .as_ref()
        .expect("observe should receive run trace context");
    assert_eq!(trace_context.node_id, "observe");
    assert!(trace_context.node_sequence > 0);
    drop(observation_options);
    let options = actions.options.lock().await;
    assert_eq!(options.len(), 1);
    assert!(options.iter().all(|option| {
        option.target_wait == TargetWaitPolicy::bounded(5_000, 100)
            && option.prepared_target.is_some()
    }));
    assert!(events.iter().any(|event| {
        event.kind == ExecutionEventKind::NodeOutputProduced
            && event.payload
                == Some(ExecutionEventPayload::NodeOutputsProduced {
                    output_names: vec!["result".to_owned()],
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

/// 构造 Start → Observe → Debug → SetValue(NodeOutput) → End 的最小数据流。
fn observe_then_write_workflow() -> WorkflowDefinition {
    workflow_definition(
        "Read then write",
        vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "observe",
                180.0,
                WorkflowNodeKind::Observe {
                    observation: ObserveSpec {
                        scope: TargetScope::Current,
                        query: AqlQuery::v3(
                            "project(first(textbox(name = \"订单号\")), fields = [text])",
                        ),
                        backend_policy: Default::default(),
                        policy: ObservationPolicy::Once,
                    },
                },
            ),
            node(
                "debug",
                360.0,
                WorkflowNodeKind::Debug {
                    value: ValueExpr::node("observe", "/result/value/value/0/text"),
                },
            ),
            node(
                "write",
                540.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::SetValue {
                        target: AutomationTarget::query(AqlQuery::v3(
                            "first(textbox(name = \"订单编号\"))",
                        )),
                        value: ValueExpr::node("observe", "/result/value/value/0/text"),
                    },
                },
            ),
            node("end", 720.0, WorkflowNodeKind::End),
            node("fail", 720.0, WorkflowNodeKind::Fail),
        ],
        vec![
            edge("start", "observe", None),
            edge("observe", "debug", Some("known")),
            edge("observe", "fail", Some("unknown")),
            edge("debug", "write", None),
            edge("write", "end", None),
        ],
    )
}

/// 创建固定纵坐标的测试节点。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        size: argusflow_core::Size {
            width: 142.0,
            height: 52.0,
        },
        definition: kind.into(),
        output_bindings: Default::default(),
    }
}

/// 创建线性测试边。
fn edge(source: &str, target: &str, branch: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: branch.map(ControlPortId::new),
    }
}
