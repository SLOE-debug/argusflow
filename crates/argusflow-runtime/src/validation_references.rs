use std::collections::{HashMap, HashSet};

use argusflow_core::{
    CommandOperation, ResourceRef, TargetScope, UiOperation, ValueExpr, WorkflowDefinition,
    WorkflowNode, WorkflowNodeKind,
};

use crate::{ValidationIssue, ValidationIssueCode, validator::issue};

/// 校验 ValueExpr、ResourceRef 的生产端口、节点存在性与 CFG 支配关系。
pub(crate) fn validate_data_references(
    workflow: &WorkflowDefinition,
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
        match &consumer.kind {
            WorkflowNodeKind::Ui { operation } => {
                validate_scope(
                    operation.target().scope.clone(),
                    consumer,
                    &nodes,
                    &dominators,
                    issues,
                );
                if let UiOperation::SetValue { value, .. } = operation {
                    validate_value(value, consumer, workflow, &nodes, &dominators, issues);
                }
            }
            WorkflowNodeKind::Command { operation } => {
                for expression in command_values(operation) {
                    validate_value(expression, consumer, workflow, &nodes, &dominators, issues);
                }
            }
            WorkflowNodeKind::Debug { value } => {
                validate_value(value, consumer, workflow, &nodes, &dominators, issues);
            }
            WorkflowNodeKind::Start
            | WorkflowNodeKind::Log { .. }
            | WorkflowNodeKind::Delay { .. }
            | WorkflowNodeKind::Condition { .. }
            | WorkflowNodeKind::Application { .. }
            | WorkflowNodeKind::End => {}
        }
    }
}

/// 校验一个 UI 资源作用域只指向支配消费者的 Application.session。
fn validate_scope(
    scope: TargetScope,
    consumer: &WorkflowNode,
    nodes: &HashMap<&str, &WorkflowNode>,
    dominators: &HashMap<String, HashSet<String>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let TargetScope::Application { resource } = scope else {
        return;
    };
    let Some(producer) = nodes.get(resource.producer_node_id.as_str()) else {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            &resource,
            "资源生产节点不存在",
        ));
        return;
    };
    if resource.output_name != "session"
        || !matches!(&producer.kind, WorkflowNodeKind::Application { .. })
    {
        issues.push(reference_issue(
            ValidationIssueCode::InvalidResourceReference,
            consumer,
            &resource,
            "引用不是 Application 节点的 session 资源端口",
        ));
        return;
    }
    validate_dominance(
        &resource.producer_node_id,
        consumer,
        dominators,
        ValidationIssueCode::ReferenceNotDominating,
        format!(
            "应用资源 '{}.{}' 并非在所有到达消费节点的路径上先执行",
            resource.producer_node_id, resource.output_name,
        ),
        issues,
    );
}

/// 校验一个值表达式的数据来源与生产端口。
fn validate_value(
    expression: &ValueExpr,
    consumer: &WorkflowNode,
    workflow: &WorkflowDefinition,
    nodes: &HashMap<&str, &WorkflowNode>,
    dominators: &HashMap<String, HashSet<String>>,
    issues: &mut Vec<ValidationIssue>,
) {
    match expression {
        ValueExpr::Literal { value } => {
            if !value.is_string() {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    "当前节点参数要求字符串字面量",
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::WorkflowInput { key } => {
            if key.trim().is_empty() || !workflow.inputs.iter().any(|input| input.key == *key) {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("工作流输入 '{key}' 没有声明"),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::Variable { name } => {
            if name.trim().is_empty()
                || !workflow
                    .variables
                    .get(name.as_str())
                    .is_some_and(serde_json::Value::is_string)
            {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("运行变量 '{name}' 不存在或不是字符串"),
                    Some(consumer.id.clone()),
                    None,
                ));
            }
        }
        ValueExpr::NodeOutput { node_id, output } => {
            let Some(producer) = nodes.get(node_id.as_str()) else {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("值输出生产节点 '{node_id}' 不存在"),
                    Some(consumer.id.clone()),
                    None,
                ));
                return;
            };
            if !node_exposes_text(producer, output) {
                issues.push(issue(
                    ValidationIssueCode::InvalidValueReference,
                    format!("节点 '{node_id}' 不公开文本输出端口 '{output}'"),
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

/// 判断节点是否公开可以直接供文本参数消费的输出端口。
fn node_exposes_text(node: &WorkflowNode, output: &str) -> bool {
    match &node.kind {
        WorkflowNodeKind::Ui {
            operation: UiOperation::GetText { .. },
        } => output == "text",
        WorkflowNodeKind::Ui {
            operation: UiOperation::GetValue { .. },
        } => output == "value",
        WorkflowNodeKind::Command { .. } => matches!(output, "stdout" | "stderr"),
        WorkflowNodeKind::Start
        | WorkflowNodeKind::Log { .. }
        | WorkflowNodeKind::Debug { .. }
        | WorkflowNodeKind::Delay { .. }
        | WorkflowNodeKind::Condition { .. }
        | WorkflowNodeKind::Application { .. }
        | WorkflowNodeKind::Ui { .. }
        | WorkflowNodeKind::End => false,
    }
}

/// 按稳定字段顺序枚举 CommandOperation 内的全部 ValueExpr。
fn command_values(operation: &CommandOperation) -> Vec<&ValueExpr> {
    operation
        .program
        .iter()
        .chain(operation.arguments.iter())
        .chain(operation.script.iter())
        .chain(operation.working_directory.iter())
        .chain(operation.environment.iter().map(|binding| &binding.value))
        .chain(operation.stdin.iter())
        .collect()
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
