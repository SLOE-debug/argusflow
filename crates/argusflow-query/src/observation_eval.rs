//! 后端实体事实上的统一投影、聚合与强三值逻辑求值。

use std::collections::BTreeMap;

use argusflow_core::{
    BackendKind, EntityField, EntityObservation, EntitySnapshot, NumberComparison, NumberOperand,
    ObservationExpr, ObservationQuery, ObservationResult, ObservationUnknownReason,
    ObservationValue,
};
use serde_json::{Value, json};

/// 按表达式稳定遍历顺序返回需要由同一后端在同一观察中求值的 selector 叶节点。
pub fn observation_selectors(query: &ObservationQuery) -> Vec<&argusflow_core::UiQuery> {
    let mut selectors = Vec::new();
    collect_selectors(&query.expression, &mut selectors);
    selectors
}

/// 使用一个后端返回的有序叶节点事实求值完整 AQL v3 表达式。
pub fn evaluate_observation(
    query: &ObservationQuery,
    observations: &[EntityObservation],
    backend: BackendKind,
) -> ObservationResult {
    if observations.len() != observation_selectors(query).len() {
        return unknown(backend, ObservationUnknownReason::InvalidResponse, false);
    }
    let mut cursor = observations.iter();
    let evaluation = evaluate_expression(&query.expression, &mut cursor, backend);
    if cursor.next().is_some() {
        return unknown(backend, ObservationUnknownReason::InvalidResponse, false);
    }
    evaluation
}

/// 递归收集 selector，组合器不会改变声明顺序。
fn collect_selectors<'query>(
    expression: &'query ObservationExpr,
    selectors: &mut Vec<&'query argusflow_core::UiQuery>,
) {
    match expression {
        ObservationExpr::Entities { query }
        | ObservationExpr::Project { query, .. }
        | ObservationExpr::Count { query }
        | ObservationExpr::Exists { query } => selectors.push(query),
        ObservationExpr::Compare { left, .. } => collect_selectors(left, selectors),
        ObservationExpr::AllOf { expressions } | ObservationExpr::AnyOf { expressions } => {
            for expression in expressions {
                collect_selectors(expression, selectors);
            }
        }
        ObservationExpr::Not { expression } => collect_selectors(expression, selectors),
    }
}

/// 对一个表达式执行严格类型和值传播。
fn evaluate_expression(
    expression: &ObservationExpr,
    observations: &mut std::slice::Iter<'_, EntityObservation>,
    backend: BackendKind,
) -> ObservationResult {
    match expression {
        ObservationExpr::Entities { .. } => {
            exact_entities(next_observation(observations, backend), backend)
        }
        ObservationExpr::Project { fields, .. } => {
            let observation = next_observation(observations, backend);
            if !observation.complete {
                return unknown(backend, ObservationUnknownReason::IncompleteCoverage, true);
            }
            known(
                backend,
                ObservationValue::Records(
                    observation
                        .entities
                        .iter()
                        .map(|entity| project_entity(entity, fields))
                        .collect(),
                ),
            )
        }
        ObservationExpr::Count { .. } => {
            let observation = next_observation(observations, backend);
            if !observation.complete {
                return unknown(backend, ObservationUnknownReason::IncompleteCoverage, true);
            }
            let Ok(count) = u64::try_from(observation.entities.len()) else {
                return unknown(backend, ObservationUnknownReason::InvalidResponse, false);
            };
            known(backend, ObservationValue::Number(count))
        }
        ObservationExpr::Exists { .. } => {
            let observation = next_observation(observations, backend);
            if !observation.entities.is_empty() {
                known(backend, ObservationValue::Boolean(true))
            } else if observation.complete {
                known(backend, ObservationValue::Boolean(false))
            } else {
                unknown(backend, ObservationUnknownReason::IncompleteCoverage, true)
            }
        }
        ObservationExpr::Compare {
            left,
            operator,
            right,
        } => match evaluate_expression(left, observations, backend) {
            ObservationResult::Known {
                value: ObservationValue::Number(left),
                ..
            } => {
                let NumberOperand::Literal(right) = right else {
                    return unknown(backend, ObservationUnknownReason::InvalidResponse, false);
                };
                known(
                    backend,
                    ObservationValue::Boolean(compare_numbers(left, *operator, *right)),
                )
            }
            ObservationResult::Known { .. } => {
                unknown(backend, ObservationUnknownReason::InvalidResponse, false)
            }
            unknown => unknown,
        },
        ObservationExpr::AllOf { expressions } => {
            evaluate_boolean_list(expressions, observations, backend, true)
        }
        ObservationExpr::AnyOf { expressions } => {
            evaluate_boolean_list(expressions, observations, backend, false)
        }
        ObservationExpr::Not { expression } => {
            match evaluate_expression(expression, observations, backend) {
                ObservationResult::Known {
                    value: ObservationValue::Boolean(value),
                    ..
                } => known(backend, ObservationValue::Boolean(!value)),
                ObservationResult::Known { .. } => {
                    unknown(backend, ObservationUnknownReason::InvalidResponse, false)
                }
                unknown => unknown,
            }
        }
    }
}

/// 返回下一个叶节点事实；长度已在入口验证。
fn next_observation<'facts>(
    observations: &mut std::slice::Iter<'facts, EntityObservation>,
    _backend: BackendKind,
) -> &'facts EntityObservation {
    observations
        .next()
        .expect("observation count was validated before evaluation")
}

/// 精确实体集合要求完整覆盖。
fn exact_entities(observation: &EntityObservation, backend: BackendKind) -> ObservationResult {
    if observation.complete {
        known(
            backend,
            ObservationValue::Entities(observation.entities.clone()),
        )
    } else {
        unknown(backend, ObservationUnknownReason::IncompleteCoverage, true)
    }
}

/// `all_of` 的 false 与 `any_of` 的 true 是部分事实下的支配值。
fn evaluate_boolean_list(
    expressions: &[ObservationExpr],
    observations: &mut std::slice::Iter<'_, EntityObservation>,
    backend: BackendKind,
    all: bool,
) -> ObservationResult {
    let mut saw_unknown = None;
    let mut values = Vec::with_capacity(expressions.len());
    for expression in expressions {
        match evaluate_expression(expression, observations, backend) {
            ObservationResult::Known {
                value: ObservationValue::Boolean(value),
                ..
            } => values.push(value),
            ObservationResult::Known { .. } => {
                saw_unknown = Some(unknown(
                    backend,
                    ObservationUnknownReason::InvalidResponse,
                    false,
                ));
            }
            value @ ObservationResult::Unknown { .. } => saw_unknown = Some(value),
        }
    }
    let dominant = if all {
        values.iter().any(|value| !value).then_some(false)
    } else {
        values.iter().any(|value| *value).then_some(true)
    };
    if let Some(value) = dominant {
        known(backend, ObservationValue::Boolean(value))
    } else if let Some(unknown) = saw_unknown {
        unknown
    } else {
        known(backend, ObservationValue::Boolean(all))
    }
}

/// 将一个实体限制到用户声明的固定字段。
fn project_entity(entity: &EntitySnapshot, fields: &[EntityField]) -> BTreeMap<String, Value> {
    fields
        .iter()
        .map(|field| {
            let (name, value) = match field {
                EntityField::Name => ("name", option_text(&entity.name)),
                EntityField::Text => ("text", option_text(&entity.text)),
                EntityField::Value => ("value", option_text(&entity.value)),
                EntityField::Role => ("role", option_text(&entity.role)),
                EntityField::Bounds => (
                    "bounds",
                    entity
                        .bounds
                        .as_ref()
                        .map_or(Value::Null, |bounds| json!(bounds)),
                ),
                EntityField::Confidence => (
                    "confidence",
                    entity.confidence.map_or(Value::Null, |value| json!(value)),
                ),
                EntityField::Source => ("source", json!(entity.source)),
            };
            (name.to_owned(), value)
        })
        .collect()
}

/// 将可选文本字段转换成显式 null 或字符串。
fn option_text(value: &Option<String>) -> Value {
    value.clone().map_or(Value::Null, Value::String)
}

/// 对非负整数执行无隐式转换的比较。
const fn compare_numbers(left: u64, operator: NumberComparison, right: u64) -> bool {
    match operator {
        NumberComparison::Equal => left == right,
        NumberComparison::NotEqual => left != right,
        NumberComparison::GreaterThan => left > right,
        NumberComparison::GreaterThanOrEqual => left >= right,
        NumberComparison::LessThan => left < right,
        NumberComparison::LessThanOrEqual => left <= right,
    }
}

/// 创建 Known 结果。
fn known(backend: BackendKind, value: ObservationValue) -> ObservationResult {
    ObservationResult::Known { backend, value }
}

/// 创建不携带业务数据的 Unknown 结果。
fn unknown(
    backend: BackendKind,
    reason: ObservationUnknownReason,
    retryable: bool,
) -> ObservationResult {
    ObservationResult::Unknown {
        backend: Some(backend),
        reason,
        retryable,
    }
}
