//! 类型声明和可以安全序列化的数据值；资源不属于数据值。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 工作流支持的封闭数据类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "of",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ValueType {
    /// 逻辑状态。
    Bool,
    /// 有符号 64 位整数，运算检查溢出。
    Int,
    /// 有限 IEEE 754 浮点。
    Float,
    /// Unicode 文字。
    Text,
    /// 元素类型固定的有序列表。
    List(Box<ValueType>),
    /// 字段集合及每个字段类型固定的记录。
    Record(BTreeMap<String, ValueType>),
    /// 显式的有值或无值。
    Optional(Box<ValueType>),
}

impl ValueType {
    /// 类型有固定嵌套和字段预算，空列表和空可选值也检查其完整声明。
    pub fn is_valid(&self) -> bool {
        fn valid(ty: &ValueType, depth: usize) -> bool {
            if depth > 48 {
                return false;
            }
            match ty {
                ValueType::List(inner) | ValueType::Optional(inner) => valid(inner, depth + 1),
                ValueType::Record(fields) => {
                    fields.len() <= 1024
                        && fields.iter().all(|(name, ty)| {
                            !name.trim().is_empty() && name.len() <= 256 && valid(ty, depth + 1)
                        })
                }
                _ => true,
            }
        }
        valid(self, 0)
    }
}

/// 按值传递的运行数据；构造后仍需在公共入口检查类型及预算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Value {
    /// 布尔值。
    Bool(bool),
    /// 整数。
    Int(i64),
    /// 必须为有限数。
    Float(f64),
    /// 文字。
    Text(String),
    /// 类型由声明或表达式确定，空列表也必须有类型。
    List(Vec<Value>),
    /// 字段记录。
    Record(BTreeMap<String, Value>),
    /// 显式可选值。
    Optional(Option<Box<Value>>),
}

impl Value {
    /// 检查结构、有限数及固定的类型嵌套深度，不执行隐式转换。
    pub fn matches(&self, expected: &ValueType) -> bool {
        fn check(value: &Value, ty: &ValueType, depth: usize) -> bool {
            if depth > 64 {
                return false;
            }
            match (value, ty) {
                (Value::Bool(_), ValueType::Bool)
                | (Value::Int(_), ValueType::Int)
                | (Value::Text(_), ValueType::Text) => true,
                (Value::Float(v), ValueType::Float) => v.is_finite(),
                (Value::List(values), ValueType::List(item)) => {
                    values.iter().all(|v| check(v, item, depth + 1))
                }
                (Value::Record(values), ValueType::Record(fields)) => {
                    values.len() == fields.len()
                        && fields.iter().all(|(key, ty)| {
                            values.get(key).is_some_and(|v| check(v, ty, depth + 1))
                        })
                }
                (Value::Optional(value), ValueType::Optional(inner)) => {
                    value.as_ref().is_none_or(|v| check(v, inner, depth + 1))
                }
                _ => false,
            }
        }
        check(self, expected, 0)
    }

    /// 估算拥有的数据空间并限制深度；溢出返回 None。
    pub fn measured_bytes(&self) -> Option<usize> {
        fn measure(value: &Value, depth: usize) -> Option<usize> {
            if depth > 64 {
                return None;
            }
            let payload = match value {
                Value::Text(v) => v.len(),
                Value::List(v) => v
                    .iter()
                    .try_fold(0usize, |sum, v| sum.checked_add(measure(v, depth + 1)?))?,
                Value::Record(v) => v.iter().try_fold(0usize, |sum, (k, v)| {
                    sum.checked_add(k.len())?
                        .checked_add(measure(v, depth + 1)?)
                })?,
                Value::Optional(Some(v)) => measure(v, depth + 1)?,
                _ => 8,
            };
            payload.checked_add(std::mem::size_of::<Value>())
        }
        measure(self, 0)
    }
}

/// 参数、记录字段和节点输出的类型清单。
pub type Fields = BTreeMap<String, ValueType>;
/// 一组拥有所有权的数据值。
pub type Values = BTreeMap<String, Value>;
