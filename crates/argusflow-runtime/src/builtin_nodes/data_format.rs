use std::sync::Arc;

use argusflow_core::{DelimitedTextFormat, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    NodeExecution, NodeFlow, NodeOutcome, NodeValidationContext, PreparedNode, RunContext,
    RuntimeError, ValidationIssue, ValidationIssueCode, ValueInput, ValueTypeId,
};

/// Delimited Text 节点的强类型 payload。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DataFormatPayload {
    /// 对象数组与确定的字段/行格式契约。
    operation: DelimitedTextFormat,
}

/// 把格式化 payload 冻结为纯数据节点。
pub(super) fn prepare(payload: DataFormatPayload) -> Arc<dyn PreparedNode> {
    Arc::new(DataFormatNode {
        operation: payload.operation,
    })
}

/// 不访问外部资源的结构化数组文本格式化节点。
#[derive(Debug)]
struct DataFormatNode {
    /// 输入表达式和输出格式。
    operation: DelimitedTextFormat,
}

#[async_trait]
impl PreparedNode for DataFormatNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Format Delimited Text".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let mut fields = std::collections::HashSet::new();
        if self.operation.fields.is_empty()
            || self
                .operation
                .fields
                .iter()
                .any(|field| field.trim().is_empty() || !fields.insert(field.as_str()))
        {
            issues.push(context.issue(
                ValidationIssueCode::InvalidDataFormat,
                "文本格式化字段必须非空且唯一",
            ));
        }
        if self.operation.row_separator.is_empty() {
            issues.push(context.issue(
                ValidationIssueCode::InvalidDataFormat,
                "文本格式化行分隔符不能为空",
            ));
        }
        issues
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        vec![ValueInput::json(&self.operation.items)]
    }

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        (name == "text").then(ValueTypeId::text)
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let value = context.resolve_value(&self.operation.items)?;
        let items = value
            .as_array()
            .ok_or_else(|| RuntimeError::ValueTypeMismatch {
                expected: "array",
                actual: json_type_name(&value),
            })?;
        let mut rows = Vec::with_capacity(items.len() + usize::from(self.operation.include_header));
        if self.operation.include_header {
            rows.push(self.operation.fields.join(&self.operation.column_separator));
        }
        for item in items {
            let object = item
                .as_object()
                .ok_or_else(|| RuntimeError::ValueTypeMismatch {
                    expected: "object",
                    actual: json_type_name(item),
                })?;
            let values = self
                .operation
                .fields
                .iter()
                .map(|field| scalar_text(object.get(field).unwrap_or(&Value::Null)))
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(values.join(&self.operation.column_separator));
        }
        let text = if rows.is_empty() {
            String::new()
        } else {
            format!(
                "{}{}",
                rows.join(&self.operation.row_separator),
                self.operation.row_separator
            )
        };
        Ok(NodeExecution {
            outcome: NodeOutcome::values(std::collections::BTreeMap::from([(
                "text".to_owned(),
                Value::String(text),
            )])),
            events: Vec::new(),
        })
    }
}

/// 只允许无歧义的标量进入分隔文本，不隐式序列化对象或数组。
fn scalar_text(value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Null => Ok(String::new()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.clone()),
        Value::Array(_) | Value::Object(_) => Err(RuntimeError::ValueTypeMismatch {
            expected: "scalar",
            actual: json_type_name(value),
        }),
    }
}

/// 返回运行时错误使用的稳定 JSON 类型名。
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
