//! 示例文档构造；每个入口仅保留运行装配。
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::collections::BTreeMap;

pub fn scope(id: &str, mut nodes: Vec<Node>, outputs: Vec<(&str, Expr)>) -> Scope {
    for index in 0..nodes.len().saturating_sub(1) {
        nodes[index].next = Some(nodes[index + 1].id.clone());
    }
    Scope {
        id: id.into(),
        entry: nodes.first().map(|node| node.id.clone()),
        nodes,
        outputs: outputs
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    }
}
pub fn workflow(name: &str, scopes: Vec<Scope>, outputs: Fields) -> Workflow {
    Workflow {
        schema_version: 1,
        name: name.into(),
        inputs: Fields::new(),
        outputs,
        resources: BTreeMap::new(),
        root: "root".into(),
        scopes,
        subflows: BTreeMap::new(),
    }
}
pub fn declare(id: &str, name: &str, value: Expr) -> Node {
    Node::new(
        id,
        Action::Let {
            name: name.into(),
            value_type: ValueType::Int,
            value,
        },
    )
}
pub fn increment(id: &str, name: &str, value: Expr) -> Node {
    Node::new(
        id,
        Action::Assign {
            assignments: vec![Assignment {
                name: name.into(),
                value: Expr::binary(BinaryOp::Add, Expr::var(name), value),
            }],
        },
    )
}
pub async fn execute(workflow: Workflow) -> Result<(), Box<dyn std::error::Error>> {
    // --json 导出同一份可运行定义，不运行任务也不写文件。
    if std::env::args().any(|arg| arg == "--json") {
        println!("{}", workflow.to_json()?);
        return Ok(());
    }
    let json = workflow.to_json()?;
    let definition = Workflow::from_json(&json)?;
    let plan = prepare(definition, &NodeRegistry::new())
        .map_err(|diagnostics| format!("{diagnostics:#?}"))?;
    let engine = WorkflowEngine::new();
    let mut run = engine.start(plan, RunInputs::default(), RunOptions::default())?;
    let result = run.wait().await?;
    if let Some(error) = &result.error {
        return Err(error.clone().into());
    }
    println!("{}", serde_json::to_string_pretty(&result.outputs)?);
    Ok(())
}
