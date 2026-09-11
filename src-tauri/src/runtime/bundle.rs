//! 从工作目录冻结可达依赖，校验草稿与磁盘修订。
use crate::document::Workspace;
use argusflow_workflow::{Action, WorkflowBundle, WorkflowId};
use std::collections::{BTreeMap, BTreeSet};

/// 从已保存文档加载完整的可达依赖，禁止运行未解决字段草稿。
pub fn load_bundle(
    workspace: &Workspace,
    root: &str,
    revisions: &BTreeMap<String, String>,
) -> Result<WorkflowBundle, String> {
    let mut pending = vec![root.to_owned()];
    let mut workflows = BTreeMap::new();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if seen.len() > 256 {
            return Err("依赖工作流超过 256 份".into());
        }
        let loaded = workspace.load(&id)?;
        if revisions
            .get(&id)
            .is_some_and(|expected| expected != &loaded.revision)
        {
            return Err(format!("conflict:{id} 已在外部修改，请重新载入后运行"));
        }
        if !loaded.file.editor.drafts.is_empty() {
            return Err(format!(
                "{} 存在未完成的字段配置",
                loaded.file.definition["name"]
            ));
        }
        let definition = crate::document::compilation::compile(&loaded.file)?;
        for scope in &definition.scopes {
            for node in &scope.nodes {
                if let Action::CallWorkflow { workflow, .. } = &node.action {
                    pending.push(workflow.0.clone());
                }
            }
        }
        workflows.insert(WorkflowId(id), definition);
    }
    Ok(WorkflowBundle {
        root: WorkflowId(root.into()),
        workflows,
    })
}
