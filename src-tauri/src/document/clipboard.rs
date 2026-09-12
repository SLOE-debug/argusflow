//! 系统剪贴板是不可信输入，先检查当前格式、图身份和结构闭包。
use super::{WorkflowFile, wire::decode_workflow};
use argusflow_workflow::Action;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "../../../tests/argusflow-desktop/unit/document/clipboard.rs"]
mod tests;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NodeClipboard {
    format: String,
    source_workflow: String,
    source_scope: String,
    selected: Vec<String>,
    bindings: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    file: WorkflowFile,
}
pub fn parse(source: &str) -> Result<NodeClipboard, String> {
    if source.len() > 16 * 1024 * 1024 {
        return Err("剪贴板超过大小限制".into());
    }
    let clipboard: NodeClipboard =
        serde_json::from_str(source).map_err(|_| "剪贴板不是工作流节点")?;
    if clipboard.format != "argusflow.nodes" || clipboard.source_workflow != clipboard.file.id {
        return Err("剪贴板格式或身份无效".into());
    }
    super::storage::validate_draft(&clipboard.file, false)?;
    let workflow = decode_workflow(&clipboard.file.definition)?;
    if workflow.scopes.len() > 4096 || workflow.root != clipboard.source_scope {
        return Err("剪贴板作用域无效".into());
    }
    let mut nodes = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    let mut owned = BTreeSet::new();
    for scope in &workflow.scopes {
        if !scopes.insert(&scope.id) {
            return Err("重复作用域身份".into());
        }
        for node in &scope.nodes {
            if !nodes.insert(&node.id) || !clipboard.file.editor.nodes.contains_key(&node.id) {
                return Err("节点身份或布局无效".into());
            }
            let children = match &node.action {
                Action::Block { scope } => vec![scope],
                Action::If {
                    then_scope,
                    else_scope,
                    ..
                } => vec![then_scope, else_scope],
                Action::While { body, .. } | Action::ForEach { body, .. } => vec![body],
                Action::Switch {
                    cases,
                    default_scope,
                    ..
                } => cases
                    .iter()
                    .map(|case| &case.scope)
                    .chain(std::iter::once(default_scope))
                    .collect(),
                Action::Try { .. } => return Err("设计器尚未提供异常捕获编辑入口".into()),
                _ => Vec::new(),
            };
            for child in children {
                if child == &workflow.root || !owned.insert(child) {
                    return Err("容器子图归属无效".into());
                }
            }
        }
    }
    if owned.len() + 1 != scopes.len() || owned.iter().any(|id| !scopes.contains(id)) {
        return Err("容器子图不完整".into());
    }
    let root = workflow
        .scopes
        .iter()
        .find(|scope| scope.id == workflow.root)
        .ok_or("缺少剪贴板根")?;
    // 起止标记只有编辑布局，各作用域使用固定键，不作为执行节点解码。
    let mut selectable: BTreeSet<String> = root.nodes.iter().map(|node| node.id.clone()).collect();
    for kind in ["start", "end"] {
        let id = format!("${kind}:{}", root.id);
        if clipboard.file.editor.nodes.contains_key(&id) {
            selectable.insert(id);
        }
    }
    if clipboard.selected.is_empty()
        || clipboard.selected.len() != selectable.len()
        || clipboard.selected.iter().collect::<BTreeSet<_>>().len() != clipboard.selected.len()
        || clipboard.selected.iter().any(|id| !selectable.contains(id))
    {
        return Err("剪贴板选择范围无效".into());
    }
    // 每个节点和作用域恰有一个拥有者，再遍历确认不存在脱离根的环。
    let mut reached = BTreeSet::from([workflow.root.as_str()]);
    for _ in 0..64 {
        let before = reached.len();
        for scope in &workflow.scopes {
            if !reached.contains(scope.id.as_str()) {
                continue;
            }
            for node in &scope.nodes {
                for child in &workflow.scopes {
                    if owned.contains(&child.id) && owns(&node.action, &child.id) {
                        reached.insert(child.id.as_str());
                    }
                }
            }
        }
        if reached.len() == before {
            break;
        }
    }
    if reached.len() != workflow.scopes.len() {
        return Err("剪贴板含孤立或过深子图".into());
    }
    Ok(clipboard)
}
fn owns(action: &Action, id: &str) -> bool {
    match action {
        Action::Block { scope } => scope == id,
        Action::If {
            then_scope,
            else_scope,
            ..
        } => then_scope == id || else_scope == id,
        Action::While { body, .. } | Action::ForEach { body, .. } => body == id,
        Action::Switch {
            cases,
            default_scope,
            ..
        } => default_scope == id || cases.iter().any(|case| case.scope == id),
        _ => false,
    }
}
