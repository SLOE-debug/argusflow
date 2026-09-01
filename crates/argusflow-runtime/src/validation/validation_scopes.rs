//! 多作用域树、While 所有权和固定边界校验。

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{FlowScope, FlowScopeBoundary, WorkflowDefinition};

use super::validator::{ValidationIssue, ValidationIssueCode, issue};
use crate::{NodeFlow, PreparedNode};

/// 校验作用域树、容器所有权、持久化尺寸和 While 资源边界。
pub(crate) fn validate_scopes(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let scopes = workflow
        .graph
        .scopes
        .iter()
        .map(|scope| (scope.id.as_str(), scope))
        .collect::<HashMap<_, _>>();
    let root = scopes.get(workflow.graph.root_scope_id.as_str()).copied();
    if root.is_none() {
        issues.push(issue(
            ValidationIssueCode::InvalidScope,
            "root_scope_id 必须指向现有根作用域",
            None,
            None,
        ));
    }
    let mut scope_ids = HashSet::new();
    let mut owners = HashSet::new();
    for scope in &workflow.graph.scopes {
        if scope.id.trim().is_empty() || !scope_ids.insert(scope.id.as_str()) {
            issues.push(issue(
                ValidationIssueCode::InvalidScope,
                "作用域 ID 必须非空且全局唯一",
                None,
                None,
            ));
        }
        validate_parent(
            workflow,
            scope,
            &scopes,
            prepared_nodes,
            &mut owners,
            issues,
        );
        validate_boundary(scope, prepared_nodes, issues);
        validate_node_sizes(scope, issues);
        validate_loop_resources(workflow, scope, prepared_nodes, issues);
    }
    validate_parent_cycles(workflow, &scopes, issues);
    validate_loop_ownership(workflow, prepared_nodes, &owners, issues);
}

/// 校验根或 While 子作用域的直接父关系。
fn validate_parent(
    workflow: &WorkflowDefinition,
    scope: &FlowScope,
    scopes: &HashMap<&str, &FlowScope>,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    owners: &mut HashSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    if scope.id == workflow.graph.root_scope_id {
        if scope.parent.is_some() || !matches!(scope.boundary, FlowScopeBoundary::Workflow { .. }) {
            issues.push(issue(
                ValidationIssueCode::InvalidScope,
                "工作流根作用域必须无父级并使用 Workflow 边界",
                None,
                None,
            ));
        }
        return;
    }
    let Some(parent) = &scope.parent else {
        issues.push(issue(
            ValidationIssueCode::InvalidScope,
            format!("子作用域 '{}' 缺少父容器", scope.id),
            None,
            None,
        ));
        return;
    };
    if !matches!(scope.boundary, FlowScopeBoundary::Loop { .. }) {
        issues.push(issue(
            ValidationIssueCode::InvalidScope,
            "工作流中的非根作用域必须使用 Loop 边界",
            Some(parent.node_id.clone()),
            None,
        ));
    }
    let parent_contains_owner = scopes
        .get(parent.scope_id.as_str())
        .is_some_and(|parent_scope| {
            parent_scope
                .nodes
                .iter()
                .any(|node| node.id == parent.node_id)
        });
    let owner_matches = prepared_nodes.get(&parent.node_id).is_some_and(|prepared| {
        matches!(prepared.flow(), NodeFlow::Loop { ref body_scope_id, .. } if body_scope_id == &scope.id)
    });
    if !parent_contains_owner || !owner_matches || !owners.insert(parent.node_id.clone()) {
        issues.push(issue(
            ValidationIssueCode::InvalidScope,
            "While 子作用域必须由直接父作用域中的唯一容器节点拥有",
            Some(parent.node_id.clone()),
            None,
        ));
    }
}

/// 校验每种作用域固定边界指向的节点及注册控制流类型。
fn validate_boundary(
    scope: &FlowScope,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let matches_flow = |node_id: &str, predicate: fn(NodeFlow) -> bool| {
        scope.nodes.iter().any(|node| node.id == node_id)
            && prepared_nodes
                .get(node_id)
                .is_some_and(|prepared| predicate(prepared.flow()))
    };
    let valid = match &scope.boundary {
        FlowScopeBoundary::Workflow { entry_node_id } => {
            matches_flow(entry_node_id, |flow| matches!(flow, NodeFlow::Start))
        }
        FlowScopeBoundary::Component { .. } => false,
        FlowScopeBoundary::Loop {
            entry_node_id,
            continue_node_id,
            complete_node_id,
        } => {
            matches_flow(entry_node_id, |flow| matches!(flow, NodeFlow::LoopEntry))
                && matches_flow(continue_node_id, |flow| {
                    matches!(flow, NodeFlow::LoopContinue)
                })
                && matches_flow(complete_node_id, |flow| {
                    matches!(flow, NodeFlow::LoopComplete)
                })
                && entry_node_id != continue_node_id
                && entry_node_id != complete_node_id
                && continue_node_id != complete_node_id
        }
    };
    if !valid {
        issues.push(issue(
            ValidationIssueCode::InvalidScopeBoundary,
            format!("作用域 '{}' 的固定边界节点不完整或类型不匹配", scope.id),
            None,
            None,
        ));
    }
    for node in &scope.nodes {
        let forbidden = match &scope.boundary {
            FlowScopeBoundary::Workflow { .. } => matches!(
                prepared_nodes.get(&node.id).map(|prepared| prepared.flow()),
                Some(NodeFlow::LoopEntry | NodeFlow::LoopContinue | NodeFlow::LoopComplete)
            ),
            FlowScopeBoundary::Loop { .. } => {
                node.definition.type_id.as_str() == "argus.start"
                    || node.definition.type_id.as_str() == "argus.end"
            }
            FlowScopeBoundary::Component { .. } => true,
        };
        if forbidden {
            issues.push(issue(
                ValidationIssueCode::InvalidScopeBoundary,
                "边界节点只能出现在与其类型匹配的作用域中",
                Some(node.id.clone()),
                None,
            ));
        }
    }
}

/// 节点尺寸属于 v10 必需契约；While 容器还必须满足可编辑最小尺寸。
fn validate_node_sizes(scope: &FlowScope, issues: &mut Vec<ValidationIssue>) {
    for node in &scope.nodes {
        let finite_positive = node.size.width.is_finite()
            && node.size.height.is_finite()
            && node.size.width > 0.0
            && node.size.height > 0.0;
        let loop_minimum = node.definition.type_id.as_str() != "argus.loop"
            || (node.size.width >= 300.0 && node.size.height >= 180.0);
        if !finite_positive || !loop_minimum {
            issues.push(issue(
                ValidationIssueCode::InvalidNodeSize,
                "节点尺寸必须为有限正数，While 容器不得小于 300×180",
                Some(node.id.clone()),
                None,
            ));
        }
    }
}

/// While 后代不得重复获取生命周期资源，但可以消费祖先已经获取的资源。
fn validate_loop_resources(
    workflow: &WorkflowDefinition,
    scope: &FlowScope,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    issues: &mut Vec<ValidationIssue>,
) {
    if scope.id == workflow.graph.root_scope_id {
        return;
    }
    for node in &scope.nodes {
        if prepared_nodes
            .get(&node.id)
            .is_some_and(|prepared| prepared.acquires_resources())
        {
            issues.push(issue(
                ValidationIssueCode::InvalidLoop,
                "请在进入 While 之前打开应用或浏览器，循环内部只能复用祖先资源",
                Some(node.id.clone()),
                None,
            ));
        }
    }
}

/// 显式沿 parent 链检测作用域树环，不受合法 While 嵌套深度影响。
fn validate_parent_cycles(
    workflow: &WorkflowDefinition,
    scopes: &HashMap<&str, &FlowScope>,
    issues: &mut Vec<ValidationIssue>,
) {
    for scope in &workflow.graph.scopes {
        let mut current = scope.id.as_str();
        let mut visited = HashSet::new();
        while current != workflow.graph.root_scope_id {
            if !visited.insert(current) {
                issues.push(issue(
                    ValidationIssueCode::InvalidScope,
                    "作用域父子关系不能形成环",
                    None,
                    None,
                ));
                break;
            }
            let Some(parent) = scopes
                .get(current)
                .and_then(|candidate| candidate.parent.as_ref())
            else {
                break;
            };
            current = &parent.scope_id;
        }
    }
}

/// 每个 While 容器都必须恰好拥有 payload 指向的一个子作用域。
fn validate_loop_ownership(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    owners: &HashSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    for node in workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
    {
        if matches!(
            prepared_nodes.get(&node.id).map(|prepared| prepared.flow()),
            Some(NodeFlow::Loop { .. })
        ) && !owners.contains(&node.id)
        {
            issues.push(issue(
                ValidationIssueCode::InvalidLoop,
                "While 容器必须拥有一个独立子作用域",
                Some(node.id.clone()),
                None,
            ));
        }
    }
}
