//! 校验同作用域和支配祖先作用域中的值与资源引用。

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{
    FlowScope, FlowScopeBoundary, ResourceRef, ValueExpr, ValueSource, WorkflowDefinition,
    WorkflowNode,
};

use super::validator::{ValidationIssue, ValidationIssueCode, issue};
use crate::{
    node_registry::{PreparedNode, ValueTypeId},
    value_runtime::validate_json_pointer,
};

/// 全部作用域共享的节点位置、支配集合与注册端口。
struct ReferenceContext<'a> {
    /// 原始工作流声明，用于核对输入和变量。
    workflow: &'a WorkflowDefinition,
    /// 全局唯一节点 ID 到节点定义。
    nodes: HashMap<&'a str, &'a WorkflowNode>,
    /// 节点所在作用域。
    node_scopes: HashMap<&'a str, &'a str>,
    /// 作用域 ID 到结构定义。
    scopes: HashMap<&'a str, &'a FlowScope>,
    /// 每个作用域内部的控制流支配集合。
    dominators: HashMap<&'a str, HashMap<String, HashSet<String>>>,
    /// 已由注册表编译的节点端口契约。
    prepared_nodes: &'a HashMap<String, Arc<dyn PreparedNode>>,
}

/// 校验值/资源输入、生产端口与跨层可见性。
pub(crate) fn validate_data_references(
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
    let nodes = workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let node_scopes = workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| {
            scope
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), scope.id.as_str()))
        })
        .collect::<HashMap<_, _>>();
    let dominators = workflow
        .graph
        .scopes
        .iter()
        .filter_map(|scope| {
            scope_entry(scope).map(|entry| {
                let node_ids = scope
                    .nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<HashSet<_>>();
                let mut predecessors = node_ids
                    .iter()
                    .map(|id| (id.clone(), Vec::new()))
                    .collect::<HashMap<_, _>>();
                for edge in &scope.edges {
                    predecessors
                        .entry(edge.target.clone())
                        .or_default()
                        .push(edge.source.clone());
                }
                (
                    scope.id.as_str(),
                    compute_dominators(&node_ids, entry, &predecessors),
                )
            })
        })
        .collect::<HashMap<_, _>>();
    let context = ReferenceContext {
        workflow,
        nodes,
        node_scopes,
        scopes,
        dominators,
        prepared_nodes,
    };
    for consumer in workflow
        .graph
        .scopes
        .iter()
        .flat_map(|scope| scope.nodes.iter())
    {
        let Some(prepared) = prepared_nodes.get(&consumer.id) else {
            continue;
        };
        for input in prepared.resource_inputs() {
            validate_resource(
                input.reference,
                input.expected_type,
                consumer,
                &context,
                issues,
            );
        }
        for input in prepared.value_inputs() {
            validate_value(
                input.expression,
                &input.expected_type,
                consumer,
                &context,
                issues,
            );
        }
        for expression in consumer.output_bindings.values() {
            validate_value(expression, &ValueTypeId::json(), consumer, &context, issues);
        }
    }
}

/// 返回作用域的固定入口节点。
fn scope_entry(scope: &FlowScope) -> Option<&str> {
    match &scope.boundary {
        FlowScopeBoundary::Workflow { entry_node_id }
        | FlowScopeBoundary::Component { entry_node_id, .. }
        | FlowScopeBoundary::Loop { entry_node_id, .. } => Some(entry_node_id),
    }
}

/// 校验资源引用指向类型匹配且在当前或祖先控制路径中严格支配消费者。
fn validate_resource(
    resource: &ResourceRef,
    expected_type: &argusflow_core::ResourceTypeId,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if !context
        .nodes
        .contains_key(resource.producer_node_id.as_str())
    {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            resource,
            "资源生产节点不存在",
        ));
        return;
    }
    let producer_matches = context
        .prepared_nodes
        .get(&resource.producer_node_id)
        .and_then(|prepared| prepared.resource_output(&resource.output_name))
        .is_some_and(|actual_type| actual_type == expected_type);
    if !producer_matches {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            resource,
            &format!("生产端口没有公开资源类型 '{}'", expected_type.as_str()),
        ));
        return;
    }
    validate_visibility(
        &resource.producer_node_id,
        consumer,
        context,
        ValidationIssueCode::ReferenceNotDominating,
        format!(
            "资源 '{}.{}' 并非在所有到达消费节点的路径上先执行",
            resource.producer_node_id, resource.output_name,
        ),
        issues,
    );
}

/// 校验一个值表达式的数据来源与注册值端口。
fn validate_value(
    expression: &ValueExpr,
    expected_type: &ValueTypeId,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    match expression {
        ValueExpr::Literal { value } if !value_matches_type(value, expected_type) => {
            issues.push(issue(
                ValidationIssueCode::InvalidValueReference,
                format!("当前节点参数要求 '{}' 字面量", expected_type.as_str()),
                Some(consumer.id.clone()),
                None,
            ))
        }
        ValueExpr::Ref { source, pointer } => {
            validate_structured_ref(source, pointer, expected_type, consumer, context, issues)
        }
        ValueExpr::Literal { .. } | ValueExpr::Expression { .. } => {}
    }
}

/// 校验结构化引用的数据源、JSON Pointer、端口和跨层支配关系。
fn validate_structured_ref(
    source: &ValueSource,
    pointer: &str,
    expected_type: &ValueTypeId,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    if !validate_json_pointer(pointer) {
        issues.push(issue(
            ValidationIssueCode::InvalidValueReference,
            format!("JSON Pointer '{pointer}' 格式无效"),
            Some(consumer.id.clone()),
            None,
        ));
        return;
    }
    match source {
        ValueSource::WorkflowInput { key } => {
            let declared = context
                .workflow
                .inputs
                .iter()
                .any(|input| input.key == *key);
            let type_matches = pointer.is_empty()
                && (expected_type == &ValueTypeId::text() || expected_type == &ValueTypeId::json());
            if key.trim().is_empty() || !declared || !type_matches {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("工作流输入 '{key}' 没有声明，或 JSON Pointer 与输入类型不匹配"),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueSource::Variable { name } => validate_variable(name, consumer, context, issues),
        ValueSource::Node { node_id } => {
            validate_node_ref(node_id, pointer, expected_type, consumer, context, issues)
        }
    }
}

/// 校验运行变量名称已经在工作流根对象中声明。
fn validate_variable(
    name: &str,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let valid = !name.trim().is_empty()
        && context
            .workflow
            .variables
            .as_object()
            .is_some_and(|variables| variables.contains_key(name));
    if !valid {
        issues.push(issue(
            if name.trim().is_empty() {
                ValidationIssueCode::InvalidValueReference
            } else {
                ValidationIssueCode::UndeclaredVariable
            },
            if name.trim().is_empty() {
                "运行变量名称不能为空".to_owned()
            } else {
                format!("变量 '{name}' 未声明")
            },
            Some(consumer.id.clone()),
            None,
        ));
    }
}

/// 校验节点 Published Outputs 的完整对象或已知第一层输出。
fn validate_node_ref(
    node_id: &str,
    pointer: &str,
    expected_type: &ValueTypeId,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(producer) = context.nodes.get(node_id) else {
        issues.push(issue(
            ValidationIssueCode::InvalidValueReference,
            format!("值输出生产节点 '{node_id}' 不存在"),
            Some(consumer.id.clone()),
            None,
        ));
        return;
    };
    if pointer.is_empty() && expected_type != &ValueTypeId::json() {
        issues.push(issue(
            ValidationIssueCode::InvalidValueReference,
            format!("节点 '{node_id}' 的完整输出对象不能作为文本参数"),
            Some(consumer.id.clone()),
            None,
        ));
        return;
    }
    if let Some((output_name, nested)) = first_pointer_token(pointer) {
        let native_type = context
            .prepared_nodes
            .get(node_id)
            .and_then(|prepared| prepared.value_output(&output_name));
        let custom_output = producer.output_bindings.contains_key(&output_name);
        if native_type.is_none() && !custom_output {
            issues.push(issue(
                ValidationIssueCode::InvalidValueReference,
                format!("节点 '{node_id}' 没有公开输出 '{output_name}'"),
                Some(consumer.id.clone()),
                None,
            ));
            return;
        }
        if !custom_output
            && nested.is_empty()
            && native_type
                .as_ref()
                .is_some_and(|actual| !types_are_compatible(actual, expected_type))
        {
            issues.push(issue(
                ValidationIssueCode::InvalidValueReference,
                format!(
                    "节点 '{node_id}' 的输出 '{output_name}' 类型不是 '{}'",
                    expected_type.as_str()
                ),
                Some(consumer.id.clone()),
                None,
            ));
            return;
        }
    }
    validate_visibility(
        node_id,
        consumer,
        context,
        ValidationIssueCode::ReferenceNotDominating,
        format!("节点输出 '{node_id}{pointer}' 不在当前作用域或支配它的祖先路径中"),
        issues,
    );
}

/// 检查生产者在同作用域支配消费者，或在祖先作用域支配进入子树的容器。
fn validate_visibility(
    producer_id: &str,
    consumer: &WorkflowNode,
    context: &ReferenceContext<'_>,
    code: ValidationIssueCode,
    message: String,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(producer_scope) = context.node_scopes.get(producer_id).copied() else {
        return;
    };
    let Some(mut current_scope) = context.node_scopes.get(consumer.id.as_str()).copied() else {
        return;
    };
    let anchor = if producer_scope == current_scope {
        consumer.id.as_str()
    } else {
        let mut anchor = None;
        while current_scope != producer_scope {
            let Some(scope) = context.scopes.get(current_scope) else {
                break;
            };
            let Some(parent) = &scope.parent else { break };
            if parent.scope_id == producer_scope {
                anchor = Some(parent.node_id.as_str());
                break;
            }
            current_scope = &parent.scope_id;
        }
        let Some(anchor) = anchor else {
            issues.push(issue(code, message, Some(consumer.id.clone()), None));
            return;
        };
        anchor
    };
    let visible = producer_id != anchor
        && context
            .dominators
            .get(producer_scope)
            .and_then(|dominators| dominators.get(anchor))
            .is_some_and(|set| set.contains(producer_id));
    if !visible {
        issues.push(issue(code, message, Some(consumer.id.clone()), None));
    }
}

/// 解码第一个 JSON Pointer token，并返回剩余嵌套路径。
fn first_pointer_token(pointer: &str) -> Option<(String, &str)> {
    let path = pointer.strip_prefix('/')?;
    let (token, nested) = path.split_once('/').unwrap_or((path, ""));
    Some((token.replace("~1", "/").replace("~0", "~"), nested))
}

/// JSON 消费者接受所有已知端口；文本消费者只接受明确文本端口。
fn types_are_compatible(actual: &ValueTypeId, expected: &ValueTypeId) -> bool {
    expected == &ValueTypeId::json() || actual == expected
}

/// 校验内置值类型；自定义类型的更细约束由 PreparedNode 校验。
fn value_matches_type(value: &serde_json::Value, expected_type: &ValueTypeId) -> bool {
    expected_type != &ValueTypeId::text() || value.is_string()
}

/// 通过经典前驱交集不动点算法计算每个节点的 dominator 集合。
fn compute_dominators(
    node_ids: &HashSet<String>,
    start_id: &str,
    predecessors: &HashMap<String, Vec<String>>,
) -> HashMap<String, HashSet<String>> {
    let mut dominators = node_ids
        .iter()
        .map(|id| {
            let initial = if id == start_id {
                HashSet::from([id.clone()])
            } else {
                node_ids.clone()
            };
            (id.clone(), initial)
        })
        .collect::<HashMap<_, _>>();
    loop {
        let mut changed = false;
        for node_id in node_ids.iter().filter(|id| id.as_str() != start_id) {
            let mut next = predecessors
                .get(node_id)
                .into_iter()
                .flatten()
                .filter_map(|predecessor| dominators.get(predecessor).cloned())
                .reduce(|left, right| left.intersection(&right).cloned().collect())
                .unwrap_or_default();
            next.insert(node_id.clone());
            if dominators.get(node_id) != Some(&next) {
                dominators.insert(node_id.clone(), next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}

/// 创建包含逻辑引用文本的资源校验问题。
fn reference_issue(
    code: ValidationIssueCode,
    consumer: &WorkflowNode,
    reference: &ResourceRef,
    reason: &str,
) -> ValidationIssue {
    issue(
        code,
        format!(
            "资源引用 '{}.{}' 无效：{reason}",
            reference.producer_node_id, reference.output_name,
        ),
        Some(consumer.id.clone()),
        None,
    )
}
