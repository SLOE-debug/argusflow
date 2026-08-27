use std::collections::{BTreeMap, HashMap};

use argusflow_core::{ValueExpr, WorkflowNode};
use serde_json::{Map, Value};

use crate::ComponentExpansionError;

/// 使用 serde 边界统一重写节点参数、输出映射和值/资源引用。
pub(crate) fn rewrite_node(
    node: &WorkflowNode,
    expanded_id: &str,
    inputs: &BTreeMap<String, ValueExpr>,
    id_map: &HashMap<String, String>,
) -> Result<WorkflowNode, ComponentExpansionError> {
    let mut value = serde_json::to_value(node).map_err(|error| ComponentExpansionError {
        node_id: Some(expanded_id.to_owned()),
        message: error.to_string(),
    })?;
    rewrite_json(&mut value, inputs, id_map)?;
    let mut rewritten =
        serde_json::from_value::<WorkflowNode>(value).map_err(|error| ComponentExpansionError {
            node_id: Some(expanded_id.to_owned()),
            message: error.to_string(),
        })?;
    rewritten.id = expanded_id.to_owned();
    Ok(rewritten)
}

/// 重写组件输出表达式中的输入占位符和内部节点 ID。
pub(crate) fn rewrite_expression(
    expression: &ValueExpr,
    inputs: &BTreeMap<String, ValueExpr>,
    id_map: &HashMap<String, String>,
) -> Result<ValueExpr, ComponentExpansionError> {
    let mut value = serde_json::to_value(expression).map_err(|error| ComponentExpansionError {
        node_id: None,
        message: error.to_string(),
    })?;
    rewrite_json(&mut value, inputs, id_map)?;
    serde_json::from_value(value).map_err(|error| ComponentExpansionError {
        node_id: None,
        message: error.to_string(),
    })
}

/// 递归改写动态 payload 中的 ValueExpr 与 ResourceRef 稳定引用。
fn rewrite_json(
    value: &mut Value,
    inputs: &BTreeMap<String, ValueExpr>,
    id_map: &HashMap<String, String>,
) -> Result<(), ComponentExpansionError> {
    match value {
        Value::Array(values) => {
            for value in values {
                rewrite_json(value, inputs, id_map)?;
            }
        }
        Value::Object(object) => {
            if let Some(replacement) = replace_input_reference(object, inputs)? {
                *value = replacement;
                rewrite_json(value, &BTreeMap::new(), id_map)?;
                return Ok(());
            }
            if object.get("type").and_then(Value::as_str) == Some("node")
                && let Some(Value::String(node_id)) = object.get_mut("node_id")
                && let Some(expanded) = id_map.get(node_id)
            {
                *node_id = expanded.clone();
            }
            if let Some(Value::String(node_id)) = object.get_mut("producer_node_id")
                && let Some(expanded) = id_map.get(node_id)
            {
                *node_id = expanded.clone();
            }
            for child in object.values_mut() {
                rewrite_json(child, inputs, id_map)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

/// 若对象是组件输入引用，则以实例绑定表达式替换，并组合 JSON Pointer。
fn replace_input_reference(
    object: &Map<String, Value>,
    inputs: &BTreeMap<String, ValueExpr>,
) -> Result<Option<Value>, ComponentExpansionError> {
    if object.get("type").and_then(Value::as_str) != Some("ref") {
        return Ok(None);
    }
    let Some(source) = object.get("source").and_then(Value::as_object) else {
        return Ok(None);
    };
    if source.get("type").and_then(Value::as_str) != Some("workflow_input") {
        return Ok(None);
    }
    let key = source
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| ComponentExpansionError {
            node_id: None,
            message: "组件输入引用缺少 key".to_owned(),
        })?;
    let binding = inputs.get(key).ok_or_else(|| ComponentExpansionError {
        node_id: None,
        message: format!("组件输入 '{key}' 没有实例绑定"),
    })?;
    let pointer = object.get("pointer").and_then(Value::as_str).unwrap_or("");
    let replacement = compose_pointer(binding, pointer)?;
    serde_json::to_value(replacement)
        .map(Some)
        .map_err(|error| ComponentExpansionError {
            node_id: None,
            message: error.to_string(),
        })
}

/// 把组件内部输入子路径组合到父流程绑定表达式。
fn compose_pointer(
    binding: &ValueExpr,
    pointer: &str,
) -> Result<ValueExpr, ComponentExpansionError> {
    if pointer.is_empty() {
        return Ok(binding.clone());
    }
    match binding {
        ValueExpr::Ref {
            source,
            pointer: parent,
        } => Ok(ValueExpr::Ref {
            source: source.clone(),
            pointer: format!("{parent}{pointer}"),
        }),
        ValueExpr::Literal { value } => value
            .pointer(pointer)
            .cloned()
            .map(|value| ValueExpr::Literal { value })
            .ok_or_else(|| ComponentExpansionError {
                node_id: None,
                message: format!("组件输入字面量不存在路径 '{pointer}'"),
            }),
        ValueExpr::Expression { .. } => Err(ComponentExpansionError {
            node_id: None,
            message: "组件输入的表达式绑定不能继续附加 JSON Pointer".to_owned(),
        }),
    }
}
