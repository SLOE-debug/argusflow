//! 第三套示例的文档构造；默认使用 CDP 替身，可显式指定浏览器路径。
use argusflow_workflow::*;
use serde_json::json;
use std::collections::BTreeMap;

fn scope(id: &str, nodes: Vec<Node>) -> Scope {
    Scope::linear(id, nodes)
}
fn task(
    id: &str,
    kind: &str,
    config: serde_json::Value,
    inputs: Vec<(&str, Expr)>,
    resources: Vec<(&str, &str)>,
    outputs: Vec<(&str, &str)>,
) -> Node {
    Node::new(
        id,
        Action::Task {
            task: Task {
                type_id: kind.into(),
                version: 1,
                config,
                inputs: inputs
                    .into_iter()
                    .map(|(name, value)| (name.into(), value))
                    .collect(),
                resources: resources
                    .into_iter()
                    .map(|(port, name)| (port.into(), name.into()))
                    .collect(),
                resource_outputs: outputs
                    .into_iter()
                    .map(|(port, name)| (port.into(), name.into()))
                    .collect(),
                retry: None,
            },
        },
    )
}
pub fn document(endpoint: &str, application: Option<&str>, browser: Option<&str>) -> Workflow {
    let mut nodes = Vec::new();
    if let Some(executable) = application {
        nodes.push(task(
            "launch-application",
            "application.launch",
            json!({"visible":false}),
            vec![
                ("executable", Expr::text(executable)),
                (
                    "arguments",
                    Expr::List {
                        item_type: ValueType::Text,
                        items: vec![],
                    },
                ),
            ],
            vec![],
            vec![("application", "app")],
        ));
    }
    let connection = if let Some(executable) = browser {
        task(
            "browser",
            "browser.launch",
            json!({"headless":true}),
            vec![("executable", Expr::text(executable))],
            vec![],
            vec![("browser", "browser")],
        )
    } else {
        task(
            "browser",
            "browser.connect",
            json!({}),
            vec![("endpoint", Expr::text(endpoint))],
            vec![],
            vec![("browser", "browser")],
        )
    };
    nodes.push(connection);
    nodes.push(Node::new(
        "try",
        Action::Try {
            body: "body".into(),
            catches: vec![Catch {
                errors: vec![ErrorKind::User],
                scope: "catch".into(),
                error_name: "error".into(),
            }],
            finally: Some("finally".into()),
        },
    ));
    let body = scope(
        "body",
        vec![
            task(
                "new-page",
                "browser.new_page",
                json!({}),
                vec![("url", Expr::text("about:blank"))],
                vec![("browser", "browser")],
                vec![("page", "page")],
            ),
            task(
                "navigate",
                "browser.navigate",
                json!({}),
                vec![("url", Expr::text("about:blank"))],
                vec![("page", "page")],
                vec![],
            ),
            Node::new(
                "expected-failure",
                Action::Fail {
                    code: "demonstrate_cleanup".into(),
                },
            ),
        ],
    );
    Workflow {
        name: "应用与浏览器作用域清理".into(),
        inputs: Fields::new(),
        outputs: Fields::new(),
        resources: BTreeMap::new(),
        root: "root".into(),
        scopes: vec![
            scope("root", nodes),
            body,
            scope("catch", vec![]),
            scope(
                "finally",
                vec![Node::new(
                    "finish",
                    Action::Wait {
                        milliseconds: Expr::int(0),
                    },
                )],
            ),
        ],
        subflows: BTreeMap::new(),
    }
}
