//! Browser 资源作用域、CDP 后端约束和链接输出端口的校验测试。

use argusflow_core::{
    AqlQuery, AutomationTarget, BackendKind, BackendPolicy, BrowserSpec, NodeEnvelope, Position,
    ResourceRef, TargetLocator, TargetScope, UiOperation, WorkflowCapabilityId, WorkflowDefinition,
    WorkflowEdge, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::validate_workflow;
use serde_json::json;
use uuid::Uuid;

/// 测试 fixture 使用的内置节点构造器。
enum WorkflowNodeKind {
    Start,
    Browser { spec: BrowserSpec },
    Ui { operation: UiOperation },
    End,
}

impl From<WorkflowNodeKind> for NodeEnvelope {
    fn from(kind: WorkflowNodeKind) -> Self {
        match kind {
            WorkflowNodeKind::Start => Self::new("argus.start", 1, json!({})),
            WorkflowNodeKind::Browser { spec } => {
                Self::new("argus.browser", 1, json!({ "spec": spec }))
            }
            WorkflowNodeKind::Ui { operation } => {
                Self::new("argus.ui", 1, json!({ "operation": operation }))
            }
            WorkflowNodeKind::End => Self::new("argus.end", 1, json!({})),
        }
    }
}

#[test]
fn validation_accepts_collect_links_from_a_dominating_browser_resource() {
    let target = AutomationTarget {
        scope: TargetScope::Browser {
            resource: ResourceRef {
                producer_node_id: "browser".to_owned(),
                output_name: "session".to_owned(),
            },
        },
        locator: TargetLocator::Query {
            query: AqlQuery::v1(
                r##"css("#hotsearch-content-wrapper a.title-content .title-content-title")"##,
            ),
        },
        backend_policy: BackendPolicy::only(BackendKind::BrowserCdp),
    };
    let workflow = WorkflowDefinition {
        schema_version: 7,
        id: Uuid::new_v4(),
        name: "Browser resource validation".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: WorkflowPermissions::from_iter([WorkflowCapabilityId::application_launch()]),
        nodes: vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "browser",
                200.0,
                WorkflowNodeKind::Browser {
                    spec: BrowserSpec {
                        executable_path: r"C:\Program Files\Google\Chrome\Application\chrome.exe"
                            .to_owned(),
                        initial_url: "https://www.baidu.com/".to_owned(),
                        launch_timeout_ms: 15_000,
                    },
                },
            ),
            node(
                "collect",
                400.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::CollectLinks { target },
                },
            ),
            node("end", 600.0, WorkflowNodeKind::End),
        ],
        edges: vec![
            edge("start", "browser"),
            edge("browser", "collect"),
            edge("collect", "end"),
        ],
    };

    assert!(validate_workflow(&workflow).valid);
}

/// 使用给定横坐标创建测试节点。
fn node(id: &str, x: f64, kind: WorkflowNodeKind) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x, y: 0.0 },
        definition: kind.into(),
    }
}

/// 创建无条件分支的线性测试边。
fn edge(source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}
