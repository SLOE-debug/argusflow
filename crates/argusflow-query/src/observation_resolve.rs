//! AQL v3 观察参数的静态类型推导与 Runtime 冻结。

use std::collections::BTreeMap;

use argusflow_core::{NumberOperand, ObservationExpr, ObservationQuery, QueryValueType, UiQuery};
use serde_json::Value;
use thiserror::Error;

use crate::{query_parameter_names, resolve_query_parameters};

/// AQL v3 参数集合无法按推导类型冻结。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObservationParameterError {
    /// 同一名称被互不兼容的上下文引用。
    #[error("AQL parameter '${name}' is required as both {first:?} and {second:?}")]
    ConflictingTypes {
        /// 不含 `$` 的参数名。
        name: String,
        /// 首次推导类型。
        first: QueryValueType,
        /// 冲突位置推导类型。
        second: QueryValueType,
    },
    /// 源码引用了未提供的绑定。
    #[error("AQL parameter '${name}' has no binding")]
    MissingBinding {
        /// 不含 `$` 的参数名。
        name: String,
    },
    /// 持久化绑定没有被源码引用。
    #[error("AQL binding '${name}' is not referenced by the query")]
    UnusedBinding {
        /// 不含 `$` 的参数名。
        name: String,
    },
    /// 已解析 JSON 值与推导类型不一致。
    #[error("AQL parameter '${name}' requires {expected:?}, got {actual}")]
    TypeMismatch {
        /// 不含 `$` 的参数名。
        name: String,
        /// 源码上下文推导类型。
        expected: QueryValueType,
        /// 不泄漏值正文的 JSON 类型名。
        actual: &'static str,
    },
}

/// 返回源码引用的参数及其唯一静态类型。
pub fn observation_parameter_types(
    query: &ObservationQuery,
) -> Result<BTreeMap<String, QueryValueType>, ObservationParameterError> {
    let mut parameters = BTreeMap::new();
    collect_expression_parameters(&query.expression, &mut parameters)?;
    Ok(parameters)
}

/// 使用 Runtime 已求值 JSON 绑定冻结 selector 文本参数和数量参数。
pub fn resolve_observation_parameters(
    query: &ObservationQuery,
    bindings: &BTreeMap<String, Value>,
) -> Result<ObservationQuery, ObservationParameterError> {
    let parameter_types = observation_parameter_types(query)?;
    if let Some(name) = parameter_types
        .keys()
        .find(|name| !bindings.contains_key(*name))
    {
        return Err(ObservationParameterError::MissingBinding { name: name.clone() });
    }
    if let Some(name) = bindings
        .keys()
        .find(|name| !parameter_types.contains_key(*name))
    {
        return Err(ObservationParameterError::UnusedBinding { name: name.clone() });
    }
    for (name, expected) in &parameter_types {
        let value = &bindings[name];
        let valid = match expected {
            QueryValueType::Text => value.is_string(),
            QueryValueType::Integer => value.as_u64().is_some(),
            QueryValueType::Boolean => value.is_boolean(),
        };
        if !valid {
            return Err(ObservationParameterError::TypeMismatch {
                name: name.clone(),
                expected: *expected,
                actual: json_type_name(value),
            });
        }
    }
    resolve_expression(&query.expression, bindings).map(ObservationQuery::new)
}

/// 递归收集选择器文本参数和数量比较参数。
fn collect_expression_parameters(
    expression: &ObservationExpr,
    parameters: &mut BTreeMap<String, QueryValueType>,
) -> Result<(), ObservationParameterError> {
    match expression {
        ObservationExpr::Entities { query }
        | ObservationExpr::Project { query, .. }
        | ObservationExpr::Count { query }
        | ObservationExpr::Exists { query } => collect_selector_parameters(query, parameters)?,
        ObservationExpr::Compare { left, right, .. } => {
            collect_expression_parameters(left, parameters)?;
            if let NumberOperand::Parameter(name) = right {
                insert_parameter(parameters, name, QueryValueType::Integer)?;
            }
        }
        ObservationExpr::AllOf { expressions } | ObservationExpr::AnyOf { expressions } => {
            for expression in expressions {
                collect_expression_parameters(expression, parameters)?;
            }
        }
        ObservationExpr::Not { expression } => {
            collect_expression_parameters(expression, parameters)?;
        }
    }
    Ok(())
}

/// 将 selector 中所有参数注册为文本类型。
fn collect_selector_parameters(
    query: &UiQuery,
    parameters: &mut BTreeMap<String, QueryValueType>,
) -> Result<(), ObservationParameterError> {
    for name in query_parameter_names(query) {
        insert_parameter(parameters, &name, QueryValueType::Text)?;
    }
    Ok(())
}

/// 插入参数类型并拒绝同名冲突。
fn insert_parameter(
    parameters: &mut BTreeMap<String, QueryValueType>,
    name: &str,
    value_type: QueryValueType,
) -> Result<(), ObservationParameterError> {
    if let Some(existing) = parameters.get(name) {
        if *existing != value_type {
            return Err(ObservationParameterError::ConflictingTypes {
                name: name.to_owned(),
                first: *existing,
                second: value_type,
            });
        }
    } else {
        parameters.insert(name.to_owned(), value_type);
    }
    Ok(())
}

/// 递归冻结所有叶节点和数量右值。
fn resolve_expression(
    expression: &ObservationExpr,
    bindings: &BTreeMap<String, Value>,
) -> Result<ObservationExpr, ObservationParameterError> {
    Ok(match expression {
        ObservationExpr::Entities { query } => ObservationExpr::Entities {
            query: resolve_selector(query, bindings)?,
        },
        ObservationExpr::Project { query, fields } => ObservationExpr::Project {
            query: resolve_selector(query, bindings)?,
            fields: fields.clone(),
        },
        ObservationExpr::Count { query } => ObservationExpr::Count {
            query: resolve_selector(query, bindings)?,
        },
        ObservationExpr::Exists { query } => ObservationExpr::Exists {
            query: resolve_selector(query, bindings)?,
        },
        ObservationExpr::Compare {
            left,
            operator,
            right,
        } => ObservationExpr::Compare {
            left: Box::new(resolve_expression(left, bindings)?),
            operator: *operator,
            right: match right {
                NumberOperand::Literal(value) => NumberOperand::Literal(*value),
                NumberOperand::Parameter(name) => NumberOperand::Literal(
                    bindings[name]
                        .as_u64()
                        .expect("binding types were validated before resolution"),
                ),
            },
        },
        ObservationExpr::AllOf { expressions } => ObservationExpr::AllOf {
            expressions: expressions
                .iter()
                .map(|expression| resolve_expression(expression, bindings))
                .collect::<Result<Vec<_>, _>>()?,
        },
        ObservationExpr::AnyOf { expressions } => ObservationExpr::AnyOf {
            expressions: expressions
                .iter()
                .map(|expression| resolve_expression(expression, bindings))
                .collect::<Result<Vec<_>, _>>()?,
        },
        ObservationExpr::Not { expression } => ObservationExpr::Not {
            expression: Box::new(resolve_expression(expression, bindings)?),
        },
    })
}

/// 使用当前 selector 实际引用的文本绑定调用既有冻结器。
fn resolve_selector(
    query: &UiQuery,
    bindings: &BTreeMap<String, Value>,
) -> Result<UiQuery, ObservationParameterError> {
    let names = query_parameter_names(query);
    let text_bindings = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                bindings[name]
                    .as_str()
                    .expect("binding types were validated before resolution")
                    .to_owned(),
            )
        })
        .collect();
    resolve_query_parameters(query, &text_bindings).map_err(|error| match error {
        crate::QueryParameterResolutionError::MissingBinding { name } => {
            ObservationParameterError::MissingBinding { name }
        }
        crate::QueryParameterResolutionError::UnusedBinding { name } => {
            ObservationParameterError::UnusedBinding { name }
        }
    })
}

/// 返回不包含业务正文的 JSON 类型名。
const fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
