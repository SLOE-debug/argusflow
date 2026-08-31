use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{ResourceRef, ValueExpr, ValueSource, WorkflowDefinition, WorkflowNode};

use super::validator::{ValidationIssue, ValidationIssueCode, issue};
use crate::{
    node_registry::{PreparedNode, ValueTypeId},
    value_runtime::validate_json_pointer,
};

/// 单次工作流校验共享的值生产者、编译节点与 CFG 索引。
struct DataReferenceContext<'a> {
    /// 原始工作流声明，用于核对输入和变量。
    workflow: &'a WorkflowDefinition,
    /// 由稳定节点 ID 索引的原始节点。
    nodes: &'a HashMap<&'a str, &'a WorkflowNode>,
    /// 已由注册表编译的节点端口契约。
    prepared_nodes: &'a HashMap<String, Arc<dyn PreparedNode>>,
    /// 每个消费节点的控制流支配集合。
    dominators: &'a HashMap<String, HashSet<String>>,
}

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
    let context = DataReferenceContext {
        workflow,
        nodes: &nodes,
        prepared_nodes,
        dominators: &dominators,
    };
    for consumer in &workflow.nodes {
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

/// 校验资源引用指向类型匹配且严格支配消费者的注册端口。
fn validate_resource(
    resource: &ResourceRef,
    expected_type: &argusflow_core::ResourceTypeId,
    consumer: &WorkflowNode,
    context: &DataReferenceContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let Some(_producer) = context.nodes.get(resource.producer_node_id.as_str()) else {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            resource,
            "资源生产节点不存在",
        ));
        return;
    };
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
            &format!("生产端口没有公开资源类型 '{}'", expected_type.as_str(),),
        ));
        return;
    }
    validate_dominance(
        &resource.producer_node_id,
        consumer,
        context.dominators,
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
    context: &DataReferenceContext<'_>,
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
        ValueExpr::Ref { source, pointer } => {
            validate_structured_ref(source, pointer, expected_type, consumer, context, issues)
        }
        ValueExpr::Expression { .. } => {
            // 高级表达式只做 prepare 语法编译，结果类型在消费节点边界检查。
        }
    }
}

/// 校验结构化引用的数据源、JSON Pointer、端口和 CFG 支配关系。
fn validate_structured_ref(
    source: &ValueSource,
    pointer: &str,
    expected_type: &ValueTypeId,
    consumer: &WorkflowNode,
    context: &DataReferenceContext<'_>,
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
        ValueSource::Variable { name } => {
            if name.trim().is_empty() {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    "运行变量名称不能为空",
                    Some(consumer.id.clone()),
                    None,
                ));
            } else if !context
                .workflow
                .variables
                .as_object()
                .is_some_and(|variables| variables.contains_key(name))
            {
                issues.push(issue(
                    ValidationIssueCode::UndeclaredVariable,
                    format!("变量 '{name}' 未声明"),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueSource::Node { node_id } => {
            validate_node_ref(node_id, pointer, expected_type, consumer, context, issues)
        }
    }
}

/// 校验节点 Published Outputs 的完整对象或已知第一层输出。
fn validate_node_ref(
    node_id: &str,
    pointer: &str,
    expected_type: &ValueTypeId,
    consumer: &WorkflowNode,
    context: &DataReferenceContext<'_>,
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
    if pointer.is_empty() {
        if expected_type != &ValueTypeId::json() {
            issues.push(issue(
                ValidationIssueCode::InvalidValueReference,
                format!("节点 '{node_id}' 的完整输出对象不能作为文本参数"),
                Some(consumer.id.clone()),
                None,
            ));
            return;
        }
    } else if let Some((output_name, nested)) = first_pointer_token(pointer) {
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
        let known_type_mismatch = !custom_output
            && nested.is_empty()
            && native_type
                .as_ref()
                .is_some_and(|actual| !types_are_compatible(actual, expected_type));
        if known_type_mismatch {
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
    validate_dominance(
        node_id,
        consumer,
        context.dominators,
        ValidationIssueCode::ReferenceNotDominating,
        format!("节点输出 '{node_id}{pointer}' 并非在所有到达消费节点的路径上先执行"),
        issues,
    );
}

/// 解码第一个 JSON Pointer token，并返回剩余嵌套路径。
fn first_pointer_token(pointer: &str) -> Option<(String, &str)> {
    let path = pointer.strip_prefix('/')?;
    let (token, nested) = path
        .split_once('/')
        .map_or((path, ""), |(token, nested)| (token, nested));
    Some((token.replace("~1", "/").replace("~0", "~"), nested))
}

/// JSON 消费者接受所有已知端口；文本消费者只接受明确文本端口。
fn types_are_compatible(actual: &ValueTypeId, expected: &ValueTypeId) -> bool {
    expected == &ValueTypeId::json() || actual == expected
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
