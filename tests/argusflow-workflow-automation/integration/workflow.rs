//! 从公开流程 API 到真实适配模块，以协议替身验证，不运行浏览器。
#[cfg(windows)]
#[path = "../unit/application.rs"]
mod application;
#[path = "../unit/browser.rs"]
mod browser;
#[path = "../support/cdp.rs"]
mod cdp;

use argusflow_runtime::*;
use argusflow_workflow::*;
use argusflow_workflow_automation::*;
use std::collections::BTreeMap;

fn task(
    id: &str,
    type_id: &str,
    config: serde_json::Value,
    inputs: Vec<(&str, Expr)>,
    resources: Vec<(&str, &str)>,
    outputs: Vec<(&str, &str)>,
) -> Node {
    Node::new(
        id,
        Action::Task {
            task: Task {
                type_id: type_id.into(),
                version: 1,
                config,
                inputs: inputs
                    .into_iter()
                    .map(|(name, value)| (name.into(), value))
                    .collect(),
                resources: resources
                    .into_iter()
                    .map(|(a, b)| (a.into(), b.into()))
                    .collect(),
                resource_outputs: outputs
                    .into_iter()
                    .map(|(a, b)| (a.into(), b.into()))
                    .collect(),
                retry: None,
            },
        },
    )
}
fn flow(mut nodes: Vec<Node>, outputs: Vec<(&str, Expr, ValueType)>) -> Workflow {
    for index in 0..nodes.len().saturating_sub(1) {
        nodes[index].next = Some(nodes[index + 1].id.clone());
    }
    let output_types = outputs
        .iter()
        .map(|(name, _, ty)| ((*name).into(), ty.clone()))
        .collect();
    Workflow {
        schema_version: 1,
        name: "adapter test".into(),
        inputs: Fields::new(),
        outputs: output_types,
        resources: BTreeMap::new(),
        root: "root".into(),
        scopes: vec![Scope {
            id: "root".into(),
            entry: nodes.first().map(|node| node.id.clone()),
            nodes,
            outputs: outputs
                .into_iter()
                .map(|(name, expr, _)| (name.into(), expr))
                .collect(),
        }],
        subflows: BTreeMap::new(),
    }
}
fn registry() -> NodeRegistry {
    let mut registry = NodeRegistry::new();
    register_automation(&mut registry, AutomationHost::default()).unwrap();
    registry
}
