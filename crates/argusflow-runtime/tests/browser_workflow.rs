//! Browser 资源作用域、CDP 事实源与 Observe 控制端口的校验测试。

use argusflow_core::{
    AqlQuery, BackendKind, BackendPolicy, BrowserSpec, ControlPortId, NodeEnvelope,
    ObservationPolicy, ObserveSpec, Position, ResourceRef, TargetScope, ValueExpr,
    WorkflowCapabilityId, WorkflowEdge, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::validate_workflow;
use serde_json::json;
mod workflow_fixture;
use workflow_fixture::workflow_definition;

/// 测试 fixture 使用的内置节点构造器。
enum WorkflowNodeKind {
    Start,
    Browser { spec: BrowserSpec },
    Observe { observation: ObserveSpec },
    Fail,
    End,
}

impl From<WorkflowNodeKind> for NodeEnvelope {
    fn from(kind: WorkflowNodeKind) -> Self {
        match kind {
            WorkflowNodeKind::Start => Self::new("argus.start", 1, json!({})),
            WorkflowNodeKind::Browser { spec } => {
                Self::new("argus.browser", 1, json!({ "spec": spec }))
            }
            WorkflowNodeKind::Observe { observation } => {
                Self::new("argus.observe", 1, json!({ "observation": observation }))
            }
            WorkflowNodeKind::Fail => Self::new(
                "argus.fail",
                1,
                json!({
                    "code": "observation_unknown",
                    "message": ValueExpr::text("浏览器事实不可用"),
                }),
            ),
            WorkflowNodeKind::End => Self::new("argus.end", 1, json!({})),
        }
    }
}

#[test]
fn validation_accepts_observation_from_a_dominating_browser_resource() {
    let observation = ObserveSpec {
        scope: TargetScope::Browser {
            resource: ResourceRef {
                producer_node_id: "browser".to_owned(),
                output_name: "session".to_owned(),
            },
        },
        query: AqlQuery::v3(r##"project(css("main a"), fields = [text, name])"##),
        backend_policy: BackendPolicy::only(BackendKind::BrowserCdp),
        policy: ObservationPolicy::Once,
    };
    let mut workflow = workflow_definition(
        "Browser resource validation",
        vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "browser",
                200.0,
                WorkflowNodeKind::Browser {
                    spec: BrowserSpec {
                        executable_path: r"C:\Program Files\Google\Chrome\Application\chrome.exe"
                            .to_owned(),
                        initial_url: "https://example.com/".to_owned(),
                        launch_timeout_ms: 15_000,
                    },
                },
            ),
            node("observe", 400.0, WorkflowNodeKind::Observe { observation }),
            node("end", 600.0, WorkflowNodeKind::End),
            node("fail", 600.0, WorkflowNodeKind::Fail),
        ],
        vec![
            edge("start", "browser", None),
            edge("browser", "observe", None),
            edge("observe", "end", Some("known")),
            edge("observe", "fail", Some("unknown")),
        ],
    );
    workflow.permissions =
        WorkflowPermissions::from_iter([WorkflowCapabilityId::application_launch()]);

    let report = validate_workflow(&workflow);
    assert!(report.valid, "{:#?}", report.issues);
}

/// 使用给定横坐标创建测试节点。
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

/// 创建无条件分支的线性测试边。
fn edge(source: &str, target: &str, branch: Option<&str>) -> WorkflowEdge {
    WorkflowEdge {
        id: format!("{source}-{target}"),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: branch.map(ControlPortId::new),
    }
}
