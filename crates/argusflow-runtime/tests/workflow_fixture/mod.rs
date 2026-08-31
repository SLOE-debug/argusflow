//! 工作流 Runtime 集成测试共享的强类型 schema v9 fixture。

// 每个集成测试会独立编译该模块，因此只使用共享 fixture 的一个职责子集。
#![allow(dead_code)]

use argusflow_core::{
    AcquirePolicy, ActivationPolicy, ApplicationSpec, CleanupPolicy, CommandOperation,
    ConditionOperator, ControlPortId, NodeEnvelope, Position, UiExecutionPolicy, UiOperation,
    ValueExpr, ValueSource, WindowTitleMatcher, WorkflowDefinition, WorkflowEdge, WorkflowNode,
    WorkflowPermissions,
};
use serde_json::json;
use uuid::Uuid;

/// 测试 fixture 使用的强类型内置节点构造器；生产契约仍只暴露 NodeEnvelope。
pub(crate) enum WorkflowNodeKind {
    Start,
    Log {
        message: String,
    },
    Debug {
        value: ValueExpr,
    },
    Delay {
        milliseconds: u64,
    },
    Condition {
        left: ValueExpr,
        operator: ConditionOperator,
        right: Option<ValueExpr>,
    },
    SetVariable {
        name: String,
        value: ValueExpr,
    },
    Application {
        spec: ApplicationSpec,
    },
    Ui {
        operation: UiOperation,
    },
    Loop {
        max_iterations: u32,
        timeout_ms: u64,
        interval_ms: u64,
    },
    Fail {
        code: String,
        message: ValueExpr,
    },
    Command {
        operation: CommandOperation,
    },
    End,
}

impl From<WorkflowNodeKind> for NodeEnvelope {
    fn from(kind: WorkflowNodeKind) -> Self {
        match kind {
            WorkflowNodeKind::Start => Self::new("argus.start", 1, json!({})),
            WorkflowNodeKind::Log { message } => {
                Self::new("argus.log", 1, json!({ "message": message }))
            }
            WorkflowNodeKind::Debug { value } => {
                Self::new("argus.debug", 1, json!({ "value": value }))
            }
            WorkflowNodeKind::Delay { milliseconds } => {
                Self::new("argus.delay", 1, json!({ "milliseconds": milliseconds }))
            }
            WorkflowNodeKind::Condition {
                left,
                operator,
                right,
            } => Self::new(
                "argus.condition",
                1,
                json!({
                    "left": left,
                    "operator": operator,
                    "right": right,
                }),
            ),
            WorkflowNodeKind::SetVariable { name, value } => Self::new(
                "argus.variable.set",
                1,
                json!({ "assignments": [{ "name": name, "value": value }] }),
            ),
            WorkflowNodeKind::Application { spec } => {
                Self::new("argus.application", 1, json!({ "spec": spec }))
            }
            WorkflowNodeKind::Ui { operation } => Self::new(
                "argus.ui",
                5,
                json!({
                    "operation": operation,
                    "execution": UiExecutionPolicy::default(),
                }),
            ),
            WorkflowNodeKind::Loop {
                max_iterations,
                timeout_ms,
                interval_ms,
            } => Self::new(
                "argus.loop",
                1,
                json!({
                    "max_iterations": max_iterations,
                    "timeout_ms": timeout_ms,
                    "interval_ms": interval_ms,
                }),
            ),
            WorkflowNodeKind::Fail { code, message } => {
                Self::new("argus.fail", 1, json!({ "code": code, "message": message }))
            }
            WorkflowNodeKind::Command { operation } => {
                Self::new("argus.command", 1, json!({ "operation": operation }))
            }
            WorkflowNodeKind::End => Self::new("argus.end", 1, json!({})),
        }
    }
}

/// 在测试中构造一条可执行的 Start -> Log -> Delay -> End 线性链。
pub(crate) fn demo_workflow(milliseconds: u64) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 9,
        id: Uuid::new_v4(),
        name: "Demo".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: no_permissions(),
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
pub(crate) fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        definition: kind.into(),
        output_bindings: Default::default(),
    }
}

/// 创建由源节点指向目标节点的测试连线，并派生稳定的连线 ID。
pub(crate) fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}

/// 构造两条分支最终汇合到 End 的条件 DAG。
pub(crate) fn condition_workflow(enabled: bool) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 9,
        id: Uuid::new_v4(),
        name: "Condition".to_owned(),
        inputs: Vec::new(),
        variables: json!({ "enabled": enabled }),
        permissions: no_permissions(),
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "condition",
                160.0,
                WorkflowNodeKind::Condition {
                    left: ValueExpr::Ref {
                        source: ValueSource::Variable {
                            name: "enabled".to_owned(),
                        },
                        pointer: String::new(),
                    },
                    operator: ConditionOperator::Equal,
                    right: Some(ValueExpr::Literal { value: json!(true) }),
                },
            ),
            node(
                "true-log",
                320.0,
                WorkflowNodeKind::Log {
                    message: "true".to_owned(),
                },
            ),
            node(
                "false-log",
                320.0,
                WorkflowNodeKind::Log {
                    message: "false".to_owned(),
                },
            ),
            node("end", 520.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "condition"),
            WorkflowEdge {
                id: "condition-true".to_owned(),
                source: "condition".to_owned(),
                target: "true-log".to_owned(),
                branch: Some(ControlPortId::new("true")),
            },
            WorkflowEdge {
                id: "condition-false".to_owned(),
                source: "condition".to_owned(),
                target: "false-log".to_owned(),
                branch: Some(ControlPortId::new("false")),
            },
            edge("true-log", "end"),
            edge("false-log", "end"),
        ],
    }
}

/// 普通工作流测试默认不授予任何命令能力。
pub(crate) fn no_permissions() -> WorkflowPermissions {
    WorkflowPermissions::default()
}

/// 构造不依赖本机文件存在性的静态应用校验契约。
pub(crate) fn test_application_spec() -> ApplicationSpec {
    ApplicationSpec {
        executable_path: r"C:\Program Files\Example\example.exe".to_owned(),
        arguments: Vec::new(),
        window_title: WindowTitleMatcher::Contains {
            value: "Example".to_owned(),
        },
        acquire_policy: AcquirePolicy::AttachOrStart,
        launch_timeout_ms: 10_000,
        cleanup_policy: CleanupPolicy::LeaveRunning,
        activation_policy: ActivationPolicy::BestEffort,
    }
}
