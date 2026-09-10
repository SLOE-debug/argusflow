use argusflow_runtime::*;
use argusflow_workflow::*;
use std::{collections::BTreeMap, sync::Arc};

pub fn node(id: &str, action: Action) -> Node {
    Node::new(id, action)
}
pub fn scope(id: &str, mut nodes: Vec<Node>, outputs: Vec<(&str, Expr)>) -> Scope {
    for index in 0..nodes.len().saturating_sub(1) {
        nodes[index].next = Some(nodes[index + 1].id.clone());
    }
    Scope {
        id: id.into(),
        entry: nodes.first().map(|node| node.id.clone()),
        nodes,
        outputs: outputs.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    }
}
pub fn workflow(scopes: Vec<Scope>, outputs: Fields) -> Workflow {
    Workflow {
        schema_version: 1,
        name: "test".into(),
        inputs: Fields::new(),
        outputs,
        resources: BTreeMap::new(),
        root: scopes[0].id.clone(),
        scopes,
        subflows: BTreeMap::new(),
    }
}
pub fn int_fields(names: &[&str]) -> Fields {
    names
        .iter()
        .map(|name| ((*name).into(), ValueType::Int))
        .collect()
}
pub fn let_int(id: &str, name: &str, value: Expr) -> Node {
    node(
        id,
        Action::Let {
            name: name.into(),
            value_type: ValueType::Int,
            value,
        },
    )
}
pub fn assign(id: &str, name: &str, value: Expr) -> Node {
    node(
        id,
        Action::Assign {
            assignments: vec![Assignment {
                name: name.into(),
                value,
            }],
        },
    )
}
pub fn add(left: Expr, right: Expr) -> Expr {
    Expr::binary(BinaryOp::Add, left, right)
}
pub fn less(left: Expr, right: Expr) -> Expr {
    Expr::binary(BinaryOp::Less, left, right)
}
pub fn output(node: &str, name: &str) -> Expr {
    Expr::NodeOutput {
        node: node.into(),
        output: name.into(),
    }
}
pub fn call(id: &str, name: &str, inputs: Vec<(&str, Expr)>) -> Node {
    node(
        id,
        Action::Call {
            subflow: name.into(),
            inputs: inputs.into_iter().map(|(n, e)| (n.into(), e)).collect(),
            resources: BTreeMap::new(),
        },
    )
}
pub async fn run(workflow: Workflow) -> Arc<RunResult> {
    let plan = prepare(workflow, &NodeRegistry::new()).unwrap_or_else(|e| panic!("{e:#?}"));
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    handle.wait().await.unwrap()
}
pub fn assert_output(result: &Arc<RunResult>, name: &str, value: i64) {
    assert_eq!(result.status, RunStatus::Completed, "{:?}", result.error);
    assert_eq!(result.outputs[name], Value::Int(value));
}
