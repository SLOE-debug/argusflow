//! 核心工作流数据契约的序列化回归测试。
//!
//! 通过 JSON 往返确认开放节点 envelope、资源引用和强类型 payload 能无损持久化。

use argusflow_core::{
    AcquirePolicy, ActivationPolicy, ApplicationSpec, AqlQuery, AutomationTarget, BackendKind,
    BackendPolicy, CleanupPolicy, CommandOperation, CommandRunner, FlowScope, FlowScopeBoundary,
    KeyChord, KeyboardKey, KeyboardModifier, NodeEnvelope, ObservationPolicy, ObservationResult,
    ObservationUnknownReason, ObserveSpec, Position, ScopedFlowGraph, Size, TargetLocator,
    TargetScope, UiOperation, ValueExpr, WindowTitleMatcher, WorkflowCapabilityId,
    WorkflowDefinition, WorkflowEdge, WorkflowInputDefinition, WorkflowInputType, WorkflowNode,
    WorkflowPermissions,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
fn workflow_contract_round_trips_through_json() {
    let workflow = WorkflowDefinition {
        schema_version: 10,
        id: Uuid::new_v4(),
        name: "契约测试".to_owned(),
        inputs: Vec::new(),
        variables: json!({ "enabled": true }),
        permissions: WorkflowPermissions::default(),
        graph: workflow_graph(
            vec![
                node("start", 0.0, "argus.start", json!({})),
                node(
                    "action",
                    240.0,
                    "argus.ui",
                    json!({
                        "operation": UiOperation::Click {
                            target: AutomationTarget::query(AqlQuery::v3(
                                "button(name = \"保存\")",
                            )),
                        },
                    }),
                ),
                node("end", 480.0, "argus.end", json!({})),
            ],
            vec![edge("start", "action"), edge("action", "end")],
        ),
    };

    let serialized = serde_json::to_string(&workflow).expect("workflow should serialize");
    let decoded: WorkflowDefinition =
        serde_json::from_str(&serialized).expect("workflow should deserialize");

    assert!(serialized.contains("\"type_id\":\"argus.ui\""));
    assert!(serialized.contains("\"language_version\":3"));
    assert!(serialized.contains("button(name = \\\"保存\\\")"));
    assert_eq!(decoded, workflow);
}

#[test]
fn schema_v10_inputs_resources_values_and_commands_round_trip_through_json() {
    let application_spec = ApplicationSpec {
        executable_path: r"C:\Program Files\Example\example.exe".to_owned(),
        arguments: vec!["--automation".to_owned()],
        window_title: WindowTitleMatcher::Contains {
            value: "Example".to_owned(),
        },
        acquire_policy: AcquirePolicy::AttachOrStart,
        launch_timeout_ms: 10_000,
        cleanup_policy: CleanupPolicy::CloseIfStartedByWorkflow,
        activation_policy: ActivationPolicy::BestEffort,
    };
    let observation = ObserveSpec {
        scope: TargetScope::Current,
        query: AqlQuery::v3("project(first(text()), fields = [text])"),
        backend_policy: BackendPolicy::only(BackendKind::WindowsUia),
        policy: ObservationPolicy::Once,
    };
    let command_operation = CommandOperation {
        runner: CommandRunner::Direct,
        program: Some(ValueExpr::text(r"C:\Windows\System32\whoami.exe")),
        arguments: vec![ValueExpr::node("read", "/text")],
        script: None,
        working_directory: None,
        environment: Vec::new(),
        stdin: None,
        timeout_ms: 30_000,
        accepted_exit_codes: vec![0],
        max_stdout_bytes: 1_048_576,
        max_stderr_bytes: 1_048_576,
    };
    let workflow = WorkflowDefinition {
        schema_version: 10,
        id: Uuid::new_v4(),
        name: "资源与数据契约".to_owned(),
        inputs: vec![WorkflowInputDefinition {
            key: "token".to_owned(),
            value_type: WorkflowInputType::Text,
        }],
        variables: json!({ "input": "ArgusFlow" }),
        permissions: WorkflowPermissions::from_iter([
            WorkflowCapabilityId::application_launch(),
            WorkflowCapabilityId::direct_command(),
        ]),
        graph: workflow_graph(
            vec![
                node("start", 0.0, "argus.start", json!({})),
                node(
                    "application",
                    180.0,
                    "argus.application",
                    json!({ "spec": application_spec }),
                ),
                node(
                    "observe",
                    360.0,
                    "argus.observe",
                    json!({ "observation": observation }),
                ),
                node(
                    "command",
                    540.0,
                    "argus.command",
                    json!({ "operation": command_operation }),
                ),
                node("end", 720.0, "argus.end", json!({})),
            ],
            vec![
                edge("start", "application"),
                edge("application", "observe"),
                edge("observe", "command"),
                edge("command", "end"),
            ],
        ),
    };

    let serialized = serde_json::to_string(&workflow).expect("schema v10 should serialize");
    let decoded: WorkflowDefinition =
        serde_json::from_str(&serialized).expect("schema v10 should deserialize");

    assert!(serialized.contains("\"type_id\":\"argus.observe\""));
    assert!(serialized.contains("\"type\":\"ref\""));
    assert!(serialized.contains("\"pointer\":\"/text\""));
    assert!(serialized.contains("\"runner\":\"direct\""));
    assert!(serialized.contains("\"value_type\":\"text\""));
    assert_eq!(decoded, workflow);
}

#[test]
fn focused_keyboard_operation_round_trips_through_json() {
    let operation = UiOperation::PressKey {
        target: AutomationTarget {
            scope: TargetScope::Current,
            locator: TargetLocator::Focused,
            backend_policy: BackendPolicy::only(BackendKind::SendInput),
        },
        chord: KeyChord {
            key: KeyboardKey::Character {
                value: "f".to_owned(),
            },
            modifiers: vec![KeyboardModifier::Control],
        },
    };

    let serialized =
        serde_json::to_string(&operation).expect("keyboard operation should serialize");
    let decoded: UiOperation =
        serde_json::from_str(&serialized).expect("keyboard operation should deserialize");

    assert!(serialized.contains("\"type\":\"press_key\""));
    assert!(serialized.contains("\"type\":\"focused\""));
    assert!(serialized.contains("\"modifiers\":[\"control\"]"));
    assert_eq!(decoded, operation);
}

#[test]
fn unknown_observation_round_trips_without_becoming_a_false_value() {
    let result = ObservationResult::Unknown {
        backend: Some(BackendKind::WindowsUia),
        reason: ObservationUnknownReason::IncompleteCoverage,
        retryable: true,
    };
    let serialized = serde_json::to_string(&result).expect("observation should serialize");
    let decoded: ObservationResult =
        serde_json::from_str(&serialized).expect("observation should deserialize");

    assert!(serialized.contains("\"status\":\"unknown\""));
    assert!(!serialized.contains("\"value\":false"));
    assert_eq!(decoded, result);
}

#[test]
fn runtime_only_visual_locator_is_not_a_persisted_variant() {
    let result = serde_json::from_value::<TargetLocator>(json!({
        "type": "visual_resolved",
        "query": {"text": "保存", "exact": true}
    }));
    assert!(result.is_err());
}

/// 以稳定布局构造 schema v10 开放节点。
fn node(id: &str, x: f64, type_id: &str, payload: Value) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        size: Size {
            width: 120.0,
            height: 52.0,
        },
        definition: NodeEnvelope::new(type_id, 1, payload),
        output_bindings: Default::default(),
    }
}

/// 把线性测试节点和边包装为当前多作用域根图。
fn workflow_graph(nodes: Vec<WorkflowNode>, edges: Vec<WorkflowEdge>) -> ScopedFlowGraph {
    ScopedFlowGraph {
        root_scope_id: "root".to_owned(),
        scopes: vec![FlowScope {
            id: "root".to_owned(),
            parent: None,
            boundary: FlowScopeBoundary::Workflow {
                entry_node_id: "start".to_owned(),
            },
            nodes,
            edges,
        }],
    }
}

/// 构造不带条件分支的线性测试边。
fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}
