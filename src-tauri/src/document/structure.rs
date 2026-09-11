//! 保存时只检查编辑器所需结构完整性，业务必填项留到运行前诊断。
use argusflow_workflow::{Action, Workflow};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate(workflow: &Workflow) -> Result<BTreeSet<&str>, String> {
    let mut scopes = BTreeMap::new();
    let mut nodes = BTreeSet::new();
    let mut children = BTreeMap::new();
    let mut owners = BTreeSet::new();
    if workflow.scopes.len() > 4096 {
        return Err("作用域超过 4096 个".into());
    }
    for scope in &workflow.scopes {
        if scope.id.is_empty() || scopes.insert(scope.id.as_str(), scope).is_some() {
            return Err("作用域身份为空或重复".into());
        }
        let mut descendants = Vec::new();
        for node in &scope.nodes {
            if node.id.is_empty() || !nodes.insert(node.id.as_str()) {
                return Err("节点身份为空或重复".into());
            }
            descendants.extend(match &node.action {
                Action::Block { scope } => vec![scope.as_str()],
                Action::If {
                    then_scope,
                    else_scope,
                    ..
                } => vec![then_scope.as_str(), else_scope.as_str()],
                Action::Switch {
                    cases,
                    default_scope,
                    ..
                } => cases
                    .iter()
                    .map(|case| case.scope.as_str())
                    .chain([default_scope.as_str()])
                    .collect(),
                Action::While { body, .. } | Action::ForEach { body, .. } => vec![body.as_str()],
                Action::Try {
                    body,
                    catches,
                    finally,
                } => std::iter::once(body.as_str())
                    .chain(catches.iter().map(|catch| catch.scope.as_str()))
                    .chain(finally.as_deref())
                    .collect(),
                _ => Vec::new(),
            });
        }
        for id in &descendants {
            if !owners.insert(*id) {
                return Err("子作用域被多个容器拥有".into());
            }
        }
        children.insert(scope.id.as_str(), descendants);
    }
    if nodes.len() > 10000 {
        return Err("节点超过 10000 个".into());
    }
    let roots = std::iter::once(workflow.root.as_str())
        .chain(workflow.subflows.values().map(|sub| sub.scope.as_str()));
    let mut reached = BTreeSet::new();
    let mut pending: Vec<_> = roots.map(|id| (id, 0)).collect();
    while let Some((id, depth)) = pending.pop() {
        if depth > 64 || !scopes.contains_key(id) || !reached.insert(id) {
            return Err("作用域缺失、重复拥有或嵌套过深".into());
        }
        pending.extend(children[id].iter().map(|child| (*child, depth + 1)));
    }
    if reached.len() != scopes.len() {
        return Err("存在孤立作用域或结构循环".into());
    }
    Ok(nodes)
}
