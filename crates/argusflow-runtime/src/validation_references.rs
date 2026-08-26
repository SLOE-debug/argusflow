use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{ResourceRef, ValueExpr, WorkflowDefinition, WorkflowNode};

use crate::{PreparedNode, ValidationIssue, ValidationIssueCode, ValueTypeId, validator::issue};

/// 校验注册节点声明的值/资源输入、生产端口、节点存在性与 CFG 支配关系。
pub(crate) fn validate_data_references(
    workflow: &WorkflowDefinition,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    start_id: &str,
    predecessors: &HashMap<String, Vec<String>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let nodes = workflow
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let dominators = compute_dominators(
        &workflow
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>(),
        start_id,
        predecessors,
    );
    for consumer in &workflow.nodes {
        let Some(prepared) = prepared_nodes.get(&consumer.id) else {
            continue;
        };
        for input in prepared.resource_inputs() {
            validate_resource(
                input.reference,
                input.expected_type,
                consumer,
                &nodes,
                prepared_nodes,
                &dominators,
                issues,
            );
        }
        for input in prepared.value_inputs() {
            validate_value(
                input.expression,
                &input.expected_type,
                consumer,
                workflow,
                &nodes,
                prepared_nodes,
                &dominators,
                issues,
            );
        }
    }
}

/// 校验资源引用指向类型匹配且严格支配消费者的注册端口。
fn validate_resource(
    resource: &ResourceRef,
    expected_type: &argusflow_core::ResourceTypeId,
    consumer: &WorkflowNode,
    nodes: &HashMap<&str, &WorkflowNode>,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    dominators: &HashMap<String, HashSet<String>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(_producer) = nodes.get(resource.producer_node_id.as_str()) else {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            resource,
            "资源生产节点不存在",
        ));
        return;
    };
    let producer_matches = prepared_nodes
        .get(&resource.producer_node_id)
        .and_then(|prepared| prepared.resource_output(&resource.output_name))
        .is_some_and(|actual_type| actual_type == expected_type);
    if !producer_matches {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            resource,
            &format!("生产端口没有公开资源类型 '{}'", expected_type.as_str(),),
        ));
        return;
    }
    validate_dominance(
        &resource.producer_node_id,
        consumer,
        dominators,
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
    workflow: &WorkflowDefinition,
    nodes: &HashMap<&str, &WorkflowNode>,
    prepared_nodes: &HashMap<String, Arc<dyn PreparedNode>>,
    dominators: &HashMap<String, HashSet<String>>,
    issues: &mut Vec<ValidationIssue>,
) {
    match expression {
        ValueExpr::Literal { value } => {
            if !value_matches_type(value, expected_type) {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("当前节点参数要求 '{}' 字面量", expected_type.as_str()),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::WorkflowInput { key } => {
            let declared_type = workflow
                .inputs
                .iter()
                .find(|input| input.key == *key)
                .map(|_| ValueTypeId::text());
            if key.trim().is_empty() || declared_type.as_ref() != Some(expected_type) {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!(
                        "工作流输入 '{key}' 没有声明或类型不是 '{}'",
                        expected_type.as_str(),
                    ),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::Variable { name } => {
            let matches_type = workflow
                .variables
                .get(name.as_str())
                .is_some_and(|value| value_matches_type(value, expected_type));
            if name.trim().is_empty() || !matches_type {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!(
                        "运行变量 '{name}' 不存在或类型不是 '{}'",
                        expected_type.as_str(),
                    ),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::NodeOutput { node_id, output } => {
            if !nodes.contains_key(node_id.as_str()) {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("值输出生产节点 '{node_id}' 不存在"),
                    Some(consumer.id.clone()),
                    None,
                ));
                return;
            }
            let exposes_expected_type = prepared_nodes
                .get(node_id)
                .and_then(|prepared| prepared.value_output(output))
                .as_ref()
                == Some(expected_type);
            if !exposes_expected_type {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!(
                        "节点 '{node_id}' 的输出端口 '{output}' 不公开类型 '{}'",
                        expected_type.as_str(),
                    ),
                    Some(consumer.id.clone()),
                    None,
                ));
                return;
            }
            validate_dominance(
                node_id,
                consumer,
                dominators,
                ValidationIssueCode::ReferenceNotDominating,
                format!("值输出 '{node_id}.{output}' 并非在所有到达消费节点的路径上先执行"),
                issues,
            );
        }
    }
}

/// 校验内置值类型；自定义类型的更细约束由拥有它的 PreparedNode 校验。
fn value_matches_type(value: &serde_json::Value, expected_type: &ValueTypeId) -> bool {
    if expected_type == &ValueTypeId::text() {
        value.is_string()
    } else {
        true
    }
}

/// 检查生产节点严格支配消费节点；节点不能引用自身尚未产生的输出。
fn validate_dominance(
    producer_id: &str,
    consumer: &WorkflowNode,
    dominators: &HashMap<String, HashSet<String>>,
    code: ValidationIssueCode,
    message: String,
    issues: &mut Vec<ValidationIssue>,
) {
    let dominates = producer_id != consumer.id
        && dominators
            .get(&consumer.id)
            .is_some_and(|set| set.contains(producer_id));
    if !dominates {
        issues.push(issue(code, message, Some(consumer.id.clone()), None));
    }
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
            let node_predecessors = predecessors
                .get(node_id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let mut next = node_predecessors
                .iter()
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
