//! 核心工作流数据契约的序列化回归测试。
//!
//! 通过 JSON 往返确认编辑器与运行时共享的结构能够无损持久化和恢复。

use argusflow_core::{
    AcquirePolicy, ActivationPolicy, ApplicationSpec, AqlQuery, AutomationTarget,
    BackendPreference, CleanupPolicy, CommandOperation, CommandRunner, Position, ResourceRef,
    TargetLocator, TargetScope, UiOperation, ValueExpr, WindowTitleMatcher, WorkflowDefinition,
    WorkflowEdge, WorkflowNode, WorkflowNodeKind, WorkflowPermissions,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn workflow_contract_round_trips_through_json() {
    // 使用包含动作选择器和多条连线的最小完整工作流，覆盖扁平化节点类型及嵌套枚举。
    let workflow = WorkflowDefinition {
        schema_version: 4,
        id: Uuid::new_v4(),
        name: "契约测试".to_owned(),
        variables: json!({ "enabled": true }),
        permissions: WorkflowPermissions {
            process_spawn: false,
            powershell: false,
            cmd: false,
        },
        nodes: vec![
            WorkflowNode {
                id: "start".to_owned(),
                position: Position { x: 0.0, y: 0.0 },
                kind: WorkflowNodeKind::Start,
            },
            WorkflowNode {
                id: "action".to_owned(),
                position: Position { x: 240.0, y: 0.0 },
                kind: WorkflowNodeKind::Ui {
                    operation: UiOperation::Click {
                        target: AutomationTarget::query(AqlQuery::v1("button(name = \"保存\")")),
                    },
                },
            },
            WorkflowNode {
                id: "end".to_owned(),
                position: Position { x: 480.0, y: 0.0 },
                kind: WorkflowNodeKind::End,
            },
        ],
        edges: vec![
            WorkflowEdge {
                id: "start-action".to_owned(),
                source: "start".to_owned(),
                target: "action".to_owned(),
                branch: None,
            },
            WorkflowEdge {
                id: "action-end".to_owned(),
                source: "action".to_owned(),
                target: "end".to_owned(),
                branch: None,
            },
        ],
    };

    let json = serde_json::to_string(&workflow).expect("workflow should serialize");
    let decoded: WorkflowDefinition =
        serde_json::from_str(&json).expect("workflow should deserialize");

    assert!(json.contains("\"language_version\":1"));
    assert!(json.contains("button(name = \\\"保存\\\")"));
    assert_eq!(decoded, workflow);
}

#[test]
fn schema_v4_resources_values_and_commands_round_trip_through_json() {
    let workflow = WorkflowDefinition {
        schema_version: 4,
        id: Uuid::new_v4(),
        name: "资源与数据契约".to_owned(),
        variables: json!({ "input": "ArgusFlow" }),
        permissions: WorkflowPermissions::direct_process_only(),
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "application",
                180.0,
                WorkflowNodeKind::Application {
                    spec: ApplicationSpec {
                        executable_path: r"C:\Program Files\Example\example.exe".to_owned(),
                        arguments: vec!["--automation".to_owned()],
                        window_title: WindowTitleMatcher::Contains {
                            value: "Example".to_owned(),
                        },
                        acquire_policy: AcquirePolicy::AttachOrStart,
                        launch_timeout_ms: 10_000,
                        cleanup_policy: CleanupPolicy::CloseIfStartedByWorkflow,
                        activation_policy: ActivationPolicy::BestEffort,
                    },
                },
            ),
            node(
                "read",
                360.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::GetText {
                        target: AutomationTarget {
                            scope: TargetScope::Application {
                                resource: ResourceRef {
                                    producer_node_id: "application".to_owned(),
                                    output_name: "session".to_owned(),
                                },
                            },
                            locator: TargetLocator::Query {
                                query: AqlQuery::v1("first(text(name = \"订单号\"))"),
                            },
                            backend_preference: BackendPreference::WindowsUia,
                        },
                    },
                },
            ),
            node(
                "command",
                540.0,
                WorkflowNodeKind::Command {
                    operation: CommandOperation {
                        runner: CommandRunner::Direct,
                        program: Some(ValueExpr::text(r"C:\Windows\System32\whoami.exe")),
                        arguments: vec![ValueExpr::NodeOutput {
                            node_id: "read".to_owned(),
                            output: "text".to_owned(),
                        }],
                        script: None,
                        working_directory: None,
                        environment: Vec::new(),
                        stdin: None,
                        timeout_ms: 30_000,
                        accepted_exit_codes: vec![0],
                        max_stdout_bytes: 1_048_576,
                        max_stderr_bytes: 1_048_576,
                    },
                },
            ),
            node("end", 720.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "application"),
            edge("application", "read"),
            edge("read", "command"),
            edge("command", "end"),
        ],
    };

    let serialized = serde_json::to_string(&workflow).expect("schema v4 should serialize");
    let decoded: WorkflowDefinition =
        serde_json::from_str(&serialized).expect("schema v4 should deserialize");

    assert!(serialized.contains("\"producer_node_id\":\"application\""));
    assert!(serialized.contains("\"type\":\"node_output\""));
    assert!(serialized.contains("\"runner\":\"direct\""));
    assert_eq!(decoded, workflow);
}

/// 以稳定布局构造序列化测试节点。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        kind,
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
