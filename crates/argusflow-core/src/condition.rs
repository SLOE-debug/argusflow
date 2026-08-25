use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// 条件节点支持的安全比较运算，不执行用户提供的代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    /// JSON 深度相等。
    Equal,
    /// JSON 深度不等。
    NotEqual,
    /// 数字大于。
    GreaterThan,
    /// 数字大于或等于。
    GreaterThanOrEqual,
    /// 数字小于。
    LessThan,
    /// 数字小于或等于。
    LessThanOrEqual,
    /// 字符串包含子串、数组包含值或对象包含键。
    Contains,
    /// 指针能够解析到值。
    Exists,
    /// 指针不能解析到值。
    NotExists,
    /// null、字符串、数组或对象为空。
    IsEmpty,
    /// null、字符串、数组或对象不为空。
    NotEmpty,
}

/// 从工作流变量中读取一个 JSON Pointer，并与可选 JSON 值比较。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionPredicate {
    /// RFC 6901 JSON Pointer；空字符串表示整个变量对象。
    pub pointer: String,
    /// 对读取结果执行的比较。
    pub operator: ConditionOperator,
    /// 二元比较的右操作数；一元运算符必须为空。
    pub operand: Option<Value>,
}

/// 条件表达式无法安全求值时的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConditionEvaluationError {
    /// 指针不符合 RFC 6901 的起始或转义规则。
    #[error("JSON Pointer '{0}' 格式无效")]
    InvalidPointer(String),
    /// 需要读取值的运算符没有在指定指针处找到值。
    #[error("JSON Pointer '{0}' 没有匹配到变量值")]
    MissingValue(String),
    /// 二元运算符缺少右操作数。
    #[error("条件运算符需要右操作数")]
    MissingOperand,
    /// 一元运算符错误地携带了右操作数。
    #[error("一元条件运算符不能携带右操作数")]
    UnexpectedOperand,
    /// 当前运算符不支持左值或右值的 JSON 类型。
    #[error("条件运算符与当前 JSON 值类型不匹配")]
    TypeMismatch,
}

impl ConditionPredicate {
    /// 对只读 JSON 变量求值。
    pub fn evaluate(&self, variables: &Value) -> Result<bool, ConditionEvaluationError> {
        validate_pointer(&self.pointer)?;
        let left = variables.pointer(&self.pointer);
        match self.operator {
            ConditionOperator::Exists => {
                ensure_no_operand(&self.operand)?;
                Ok(left.is_some())
            }
            ConditionOperator::NotExists => {
                ensure_no_operand(&self.operand)?;
                Ok(left.is_none())
            }
            ConditionOperator::IsEmpty => {
                ensure_no_operand(&self.operand)?;
                let left = required_value(left, &self.pointer)?;
                is_empty(left).ok_or(ConditionEvaluationError::TypeMismatch)
            }
            ConditionOperator::NotEmpty => {
                ensure_no_operand(&self.operand)?;
                let left = required_value(left, &self.pointer)?;
                is_empty(left)
                    .map(|empty| !empty)
                    .ok_or(ConditionEvaluationError::TypeMismatch)
            }
            ConditionOperator::Equal => {
                compare_values(left, &self.pointer, &self.operand, |left, right| {
                    left == right
                })
            }
            ConditionOperator::NotEqual => {
                compare_values(left, &self.pointer, &self.operand, |left, right| {
                    left != right
                })
            }
            ConditionOperator::GreaterThan => {
                compare_numbers(left, &self.pointer, &self.operand, |left, right| {
                    left > right
                })
            }
            ConditionOperator::GreaterThanOrEqual => {
                compare_numbers(left, &self.pointer, &self.operand, |left, right| {
                    left >= right
                })
            }
            ConditionOperator::LessThan => {
                compare_numbers(left, &self.pointer, &self.operand, |left, right| {
                    left < right
                })
            }
            ConditionOperator::LessThanOrEqual => {
                compare_numbers(left, &self.pointer, &self.operand, |left, right| {
                    left <= right
                })
            }
            ConditionOperator::Contains => contains_value(left, &self.pointer, &self.operand),
        }
    }
}

fn required_value<'a>(
    value: Option<&'a Value>,
    pointer: &str,
) -> Result<&'a Value, ConditionEvaluationError> {
    value.ok_or_else(|| ConditionEvaluationError::MissingValue(pointer.to_owned()))
}

fn required_operand(operand: &Option<Value>) -> Result<&Value, ConditionEvaluationError> {
    operand
        .as_ref()
        .ok_or(ConditionEvaluationError::MissingOperand)
}

fn compare_values(
    left: Option<&Value>,
    pointer: &str,
    operand: &Option<Value>,
    compare: impl FnOnce(&Value, &Value) -> bool,
) -> Result<bool, ConditionEvaluationError> {
    Ok(compare(
        required_value(left, pointer)?,
        required_operand(operand)?,
    ))
}

fn compare_numbers(
    left: Option<&Value>,
    pointer: &str,
    operand: &Option<Value>,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Result<bool, ConditionEvaluationError> {
    let left = required_value(left, pointer)?
        .as_f64()
        .ok_or(ConditionEvaluationError::TypeMismatch)?;
    let right = required_operand(operand)?
        .as_f64()
        .ok_or(ConditionEvaluationError::TypeMismatch)?;
    Ok(compare(left, right))
}

fn contains_value(
    left: Option<&Value>,
    pointer: &str,
    operand: &Option<Value>,
) -> Result<bool, ConditionEvaluationError> {
    let left = required_value(left, pointer)?;
    let right = required_operand(operand)?;
    match left {
        Value::String(left) => right
            .as_str()
            .map(|right| left.contains(right))
            .ok_or(ConditionEvaluationError::TypeMismatch),
        Value::Array(items) => Ok(items.iter().any(|item| item == right)),
        Value::Object(object) => right
            .as_str()
            .map(|key| object.contains_key(key))
            .ok_or(ConditionEvaluationError::TypeMismatch),
        Value::Null | Value::Bool(_) | Value::Number(_) => {
            Err(ConditionEvaluationError::TypeMismatch)
        }
    }
}

fn ensure_no_operand(operand: &Option<Value>) -> Result<(), ConditionEvaluationError> {
    if operand.is_some() {
        Err(ConditionEvaluationError::UnexpectedOperand)
    } else {
        Ok(())
    }
}

fn is_empty(value: &Value) -> Option<bool> {
    match value {
        Value::Null => Some(true),
        Value::String(value) => Some(value.is_empty()),
        Value::Array(value) => Some(value.is_empty()),
        Value::Object(value) => Some(value.is_empty()),
        Value::Bool(_) | Value::Number(_) => None,
    }
}

fn validate_pointer(pointer: &str) -> Result<(), ConditionEvaluationError> {
    if pointer.is_empty() {
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err(ConditionEvaluationError::InvalidPointer(pointer.to_owned()));
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if !matches!(bytes.get(index + 1).copied(), Some(b'0' | b'1')) {
                return Err(ConditionEvaluationError::InvalidPointer(pointer.to_owned()));
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(())
}
