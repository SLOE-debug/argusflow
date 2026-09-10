//! 参数和属性值不通过源码拼接传递。
use serde::{Deserialize, Serialize};

/// 有限的属性和参数类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    /// Unicode 字符串。
    Text,
    /// 布尔值。
    Boolean,
    /// 有限数值。
    Number,
}
/// 具备静态类型的参数或后端属性。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    /// 不解释内容的文字。
    Text(String),
    /// 布尔状态。
    Boolean(bool),
    /// 必须为有限数，入口会拒绝 NaN 和无穷。
    Number(f64),
}
impl Value {
    /// 当前值的类型。
    pub const fn value_type(&self) -> ValueType {
        match self {
            Self::Text(_) => ValueType::Text,
            Self::Boolean(_) => ValueType::Boolean,
            Self::Number(_) => ValueType::Number,
        }
    }
    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Number(n) => n.is_finite(),
            Self::Text(s) => s.len() <= 65_536,
            Self::Boolean(_) => true,
        }
    }
}
