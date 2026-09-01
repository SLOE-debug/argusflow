//! 工作流级元数据、作用域终点和问题定位校验。

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{FlowScope, FlowScopeBoundary, WorkflowDefinition};

use super::validator::{ValidationIssue, ValidationIssueCode, issue};
use crate::node_registry::{NodeFlow, PreparedNode};

/// 校验工作流级契约。
pub(super) fn validate_workflow_metadata(
    workflow: &WorkflowDefinition,
    issues: &mut Vec<ValidationIssue>,
) {
    if workflow.schema_version != 10 {
        issues.push(issue(
            ValidationIssueCode::UnsupportedSchemaVersion,
            "schema_version 必须为 10",
            None,
            None,
        ));
    }
    if workflow.name.trim().is_empty() {
        issues.push(issue(
            ValidationIssueCode::EmptyWorkflowName,
            "工作流名称不能为空",
            None,
            None,
        ));
    }
    let mut input_keys = HashSet::new();
    for input in &workflow.inputs {
        if input.key.trim().is_empty() || !input_keys.insert(input.key.as_str()) {
            issues.push(issue(
                ValidationIssueCode::InvalidWorkflowInputs,
                "工作流输入名称必须非空且唯一",
                None,
                None,
            ));
        }
    }
    if !workflow.variables.is_object() {
        issues.push(issue(
            ValidationIssueCode::InvalidVariables,
            "工作流变量根值必须是 JSON 对象",
            None,
            None,
        ));
    }
}

/// 返回一个作用域的唯一入口和所有合法终止节点。
pub(super) fn scope_terminals(
    scope: &FlowScope,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
) -> (Vec<String>, Vec<String>) {
    let entry = match &scope.boundary {
        FlowScopeBoundary::Workflow { entry_node_id }
        | FlowScopeBoundary::Component { entry_node_id, .. }
        | FlowScopeBoundary::Loop { entry_node_id, .. } => entry_node_id.clone(),
    };
    let mut ends = scope
        .nodes
        .iter()
        .filter_map(|node| {
            prepared_nodes
                .get(&node.id)
                .is_some_and(|prepared| matches!(prepared.flow(), NodeFlow::End))
                .then_some(node.id.clone())
        })
        .collect::<Vec<_>>();
    match &scope.boundary {
        FlowScopeBoundary::Loop {
            continue_node_id,
            complete_node_id,
            ..
        } => {
            ends.push(continue_node_id.clone());
            ends.push(complete_node_id.clone());
        }
        FlowScopeBoundary::Component { exit_node_id, .. } => ends.push(exit_node_id.clone()),
        FlowScopeBoundary::Workflow { .. } => {}
    }
    ends.sort();
    ends.dedup();
    (vec![entry], ends)
}

/// While 的 Continue 与 Complete 固定边界允许只连接实际使用的一侧。
pub(super) fn optional_scope_boundaries(scope: &FlowScope) -> HashSet<String> {
    match &scope.boundary {
        FlowScopeBoundary::Loop {
            continue_node_id,
            complete_node_id,
            ..
        } => HashSet::from([continue_node_id.clone(), complete_node_id.clone()]),
        FlowScopeBoundary::Workflow { .. } | FlowScopeBoundary::Component { .. } => HashSet::new(),
    }
}

/// 边 ID 在整个多作用域图中保持唯一，避免事件定位发生歧义。
pub(super) fn validate_global_edge_ids(
    workflow: &WorkflowDefinition,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut edge_ids = HashSet::new();
    for edge in workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.edges.iter())
    {
        if !edge_ids.insert(edge.id.as_str()) {
            issues.push(issue(
                ValidationIssueCode::DuplicateEdgeId,
                format!("连线 ID '{}' 在多个作用域中重复", edge.id),
                None,
                Some(edge.id.clone()),
            ));
        }
    }
}

/// 给节点/连线问题补充作用域与从根到当前层的结构路径。
pub(super) fn annotate_issue_locations(
    workflow: &WorkflowDefinition,
    issues: &mut [ValidationIssue],
) {
    let scopes = workflow
        .graph
        .scopes
        .iter()
        .map(|scope| (scope.id.as_str(), scope))
        .collect::<HashMap<_, _>>();
    for issue in issues {
        let located_scope = workflow.graph.scopes.iter().find(|scope| {
            issue
                .node_id
                .as_ref()
                .is_some_and(|id| scope.nodes.iter().any(|node| node.id == *id))
                || issue
                    .edge_id
                    .as_ref()
                    .is_some_and(|id| scope.edges.iter().any(|edge| edge.id == *id))
        });
        let Some(scope) = located_scope else { continue };
        issue.scope_id = Some(scope.id.clone());
        let mut path = vec![scope.id.clone()];
        let mut current = scope;
        let mut visited = HashSet::new();
        while let Some(parent) = &current.parent {
            if !visited.insert(current.id.as_str()) {
                break;
            }
            path.push(parent.scope_id.clone());
            let Some(parent_scope) = scopes.get(parent.scope_id.as_str()).copied() else {
                break;
            };
            current = parent_scope;
        }
        path.reverse();
        issue.structure_path = path;
    }
}

/// 校验根作用域唯一 Start 与至少一个 End/Fail。
pub(super) fn validate_terminal_counts(
    scope: &FlowScope,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    end_ids: &[String],
    issues: &mut Vec<ValidationIssue>,
) {
    let start_count = scope
        .nodes
        .iter()
        .filter(|node| {
            prepared_nodes
                .get(&node.id)
                .is_some_and(|prepared| matches!(prepared.flow(), NodeFlow::Start))
        })
        .count();
    if start_count != 1 {
        issues.push(issue(
            ValidationIssueCode::InvalidStartCount,
            "工作流必须且只能包含一个 Start 节点",
            None,
            None,
        ));
    }
    if end_ids.is_empty() {
        issues.push(issue(
            ValidationIssueCode::InvalidEndCount,
            "工作流至少需要一个 End 或 Fail 终点",
            None,
            None,
        ));
    }
}
