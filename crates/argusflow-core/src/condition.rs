use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// 条件节点支持的安全比较运算，不执行用户提供的副作用代码。
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
    /// 左表达式能够解析到值。
    Exists,
    /// 左表达式不能解析到值。
    NotExists,
    /// null、字符串、数组或对象为空。
    IsEmpty,
    /// null、字符串、数组或对象不为空。
    NotEmpty,
}

/// 条件操作数无法按运算符语义比较时的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConditionEvaluationError {
    /// 需要左值的运算符没有解析到值。
    #[error("条件左表达式没有解析到值")]
    MissingLeft,
    /// 二元运算符缺少右操作数。
    #[error("条件运算符需要右操作数")]
    MissingRight,
    /// 一元运算符错误地携带了右操作数。
    #[error("一元条件运算符不能携带右操作数")]
    UnexpectedRight,
    /// 当前运算符不支持左值或右值的 JSON 类型。
    #[error("条件运算符与当前 JSON 值类型不匹配")]
    TypeMismatch,
}

impl ConditionOperator {
    /// 一元运算符不接受右表达式。
    pub const fn is_unary(self) -> bool {
        matches!(
            self,
            Self::Exists | Self::NotExists | Self::IsEmpty | Self::NotEmpty
        )
    }

    /// 对两个已经由 Runtime 求值的 JSON 操作数执行纯比较。
    pub fn evaluate(
        self,
        left: Option<&Value>,
        right: Option<&Value>,
    ) -> Result<bool, ConditionEvaluationError> {
        match self {
            Self::Exists => {
                ensure_no_right(right)?;
                Ok(left.is_some())
            }
            Self::NotExists => {
                ensure_no_right(right)?;
                Ok(left.is_none())
            }
            Self::IsEmpty => {
                ensure_no_right(right)?;
                is_empty(required_left(left)?).ok_or(ConditionEvaluationError::TypeMismatch)
            }
            Self::NotEmpty => {
                ensure_no_right(right)?;
                is_empty(required_left(left)?)
                    .map(|empty| !empty)
                    .ok_or(ConditionEvaluationError::TypeMismatch)
            }
            Self::Equal => compare_values(left, right, |left, right| left == right),
            Self::NotEqual => compare_values(left, right, |left, right| left != right),
            Self::GreaterThan => compare_numbers(left, right, |left, right| left > right),
            Self::GreaterThanOrEqual => compare_numbers(left, right, |left, right| left >= right),
            Self::LessThan => compare_numbers(left, right, |left, right| left < right),
            Self::LessThanOrEqual => compare_numbers(left, right, |left, right| left <= right),
            Self::Contains => contains_value(required_left(left)?, required_right(right)?),
        }
    }
}

fn required_left(value: Option<&Value>) -> Result<&Value, ConditionEvaluationError> {
    value.ok_or(ConditionEvaluationError::MissingLeft)
}

fn required_right(value: Option<&Value>) -> Result<&Value, ConditionEvaluationError> {
    value.ok_or(ConditionEvaluationError::MissingRight)
}

fn compare_values(
    left: Option<&Value>,
    right: Option<&Value>,
    compare: impl FnOnce(&Value, &Value) -> bool,
) -> Result<bool, ConditionEvaluationError> {
    Ok(compare(required_left(left)?, required_right(right)?))
}

fn compare_numbers(
    left: Option<&Value>,
    right: Option<&Value>,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Result<bool, ConditionEvaluationError> {
    let left = required_left(left)?
        .as_f64()
        .ok_or(ConditionEvaluationError::TypeMismatch)?;
    let right = required_right(right)?
        .as_f64()
        .ok_or(ConditionEvaluationError::TypeMismatch)?;
    Ok(compare(left, right))
}

fn contains_value(left: &Value, right: &Value) -> Result<bool, ConditionEvaluationError> {
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

fn ensure_no_right(right: Option<&Value>) -> Result<(), ConditionEvaluationError> {
    if right.is_some() {
        Err(ConditionEvaluationError::UnexpectedRight)
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
