//! 编辑草稿到当前执行契约的显式编译；不会使用字段的旧成功配置。
use super::{WorkflowFile, wire::decode_workflow};
use argusflow_workflow::{Action, EdgeEndpoint, ScopeGraph, Workflow};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/compilation.rs"]
mod tests;

pub fn compile(file: &WorkflowFile) -> Result<Workflow, String> {
    if !file.editor.drafts.is_empty() {
        return Err("存在未完成的字段配置".into());
    }
    let mut workflow = decode_workflow(&file.definition)?;
    let terminal: BTreeSet<String> = workflow
        .scopes
        .iter()
        .filter(|scope| !can_complete(&workflow, &scope.id, &BTreeSet::new()))
        .map(|scope| scope.id.clone())
        .collect();
    // 正常出口草稿继续保存在文件里。Return/Fail 等终止路径不生成正常出口。
    for scope in &mut workflow.scopes {
        if terminal.contains(&scope.id) {
            scope.outputs.clear();
        }
    }
    Ok(workflow)
}
fn can_complete(workflow: &Workflow, id: &str, ancestors: &BTreeSet<String>) -> bool {
    if ancestors.len() >= 64 || ancestors.contains(id) {
        return true;
    }
    let Some(scope) = workflow.scopes.iter().find(|scope| scope.id == id) else {
        return true;
    };
    let mut ancestors = ancestors.clone();
    ancestors.insert(id.into());
    let completes = |id: &str| can_complete(workflow, id, &ancestors);
    let Ok(graph) = ScopeGraph::new(scope) else {
        return true;
    };
    let mut next = graph.successor(&EdgeEndpoint::Start).ok();
    let mut visited = BTreeSet::new();
    while let Some(EdgeEndpoint::Node { node: id }) = next {
        if !visited.insert(id) {
            return true;
        }
        let Some(node) = scope.nodes.iter().find(|node| node.id == *id) else {
            return true;
        };
        let normal = match &node.action {
            Action::Return { .. } | Action::Fail { .. } | Action::Break | Action::Continue => false,
            Action::Block { scope } => completes(scope),
            Action::If {
                then_scope,
                else_scope,
                ..
            } => completes(then_scope) || completes(else_scope),
            Action::Switch {
                cases,
                default_scope,
                ..
            } => completes(default_scope) || cases.iter().any(|case| completes(&case.scope)),
            Action::Try {
                body,
                catches,
                finally,
            } => {
                (completes(body) || catches.iter().any(|catch| completes(&catch.scope)))
                    && finally.as_ref().is_none_or(|scope| completes(scope))
            }
            _ => true,
        };
        if !normal {
            return false;
        }
        next = graph.successor(&EdgeEndpoint::node(id)).ok();
    }
    true
}
