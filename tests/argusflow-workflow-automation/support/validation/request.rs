//! 复用正式 Workflow 反序列化、任务注册表及 AQL 编译器。
use argusflow_runtime::{NodeRegistry, prepare};
use argusflow_workflow::Workflow;
use argusflow_workflow_automation::{AutomationHost, register_automation};
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    workflow: Option<Workflow>,
    queries: Vec<String>,
}

pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut source)?;
    if source.len() > 8 * 1024 * 1024 {
        return Err("验证输入超过 8 MiB".into());
    }
    let request: Request = serde_json::from_str(&source)?;
    for query in &request.queries {
        argusflow_aql::compile_target(query)?;
    }
    if let Some(workflow) = request.workflow {
        let mut registry = NodeRegistry::new();
        register_automation(&mut registry, AutomationHost::default())?;
        super::chat_task::register(&mut registry, None)?;
        prepare(workflow, &registry).map_err(|diagnostic| format!("{diagnostic:?}"))?;
    }
    println!("{{\"contract_valid\":true,\"executed\":false}}");
    Ok(())
}
