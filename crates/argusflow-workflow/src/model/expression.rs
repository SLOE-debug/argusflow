//! 可静态检查的表达式树，不接收脚本源码。
use crate::{Value, ValueType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 二元运算；逻辑运算短路，其他运算由操作数类型决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOp {
    /// 数值加法。
    Add,
    /// 数值减法。
    Subtract,
    /// 数值乘法。
    Multiply,
    /// 同类型数值除法。
    Divide,
    /// 整数余数。
    Remainder,
    /// 同类型值相等。
    Equal,
    /// 同类型值不等。
    NotEqual,
    /// 数值或文字比较。
    Less,
    /// 数值或文字比较。
    LessEqual,
    /// 数值或文字比较。
    Greater,
    /// 数值或文字比较。
    GreaterEqual,
    /// 布尔短路与。
    And,
    /// 布尔短路或。
    Or,
}

/// 内置纯函数，无 I/O、时钟、随机数或隐式动态求值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Function {
    /// 文字 Unicode 标量或列表元素数量。
    Length,
    /// 同类型文字或列表拼接。
    Concat,
    /// 文字包含子串，或列表包含元素。
    Contains,
    /// 文字首尾去空白。
    Trim,
    /// Unicode 小写转换。
    Lowercase,
    /// Unicode 大写转换。
    Uppercase,
    /// 以非空分隔符拆分文字。
    Split,
    /// 将文字列表以分隔符连接。
    Join,
    /// 把元素追加到列表副本。
    Append,
    /// 判断可选值是否有值。
    IsSome,
    /// 可选值缺失时使用显式默认值。
    OrElse,
    /// 显式整数转浮点，要求整数可精确表示。
    ToFloat,
    /// 标量显式转换为文字。
    ToText,
}

/// 持久化表达式；编译时完成名称绑定和类型检查。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    /// 有明确类型的常量，空列表和无值 Optional 不需要猜测类型。
    Literal {
        /// 常量类型。
        value_type: ValueType,
        /// 常量内容。
        value: Value,
    },
    /// 词法查找最近声明，包含暂时性死区。
    Variable {
        /// 变量名称。
        name: String,
    },
    /// 当前工作流或子流程的只读参数。
    Input {
        /// 参数名称。
        name: String,
    },
    /// 已完成节点的某个公开输出。
    NodeOutput {
        /// 节点 ID。
        node: String,
        /// 输出名称。
        output: String,
    },
    /// 仅在节点输出映射阶段可见的原生输出。
    Result {
        /// 原生输出名称。
        output: String,
    },
    /// 固定记录字段。
    Field {
        /// 记录表达式。
        value: Box<Expr>,
        /// 字段名。
        field: String,
    },
    /// 列表下标，越界产生表达式错误。
    Index {
        /// 列表表达式。
        value: Box<Expr>,
        /// 非负整数下标。
        index: Box<Expr>,
    },
    /// 同类型二元运算。
    Binary {
        /// 运算种类。
        op: BinaryOp,
        /// 左值。
        left: Box<Expr>,
        /// 右值。
        right: Box<Expr>,
    },
    /// 布尔取反。
    Not {
        /// 布尔表达式。
        value: Box<Expr>,
    },
    /// 条件选择，只求值命中的分支。
    Choose {
        /// 布尔条件。
        condition: Box<Expr>,
        /// 真分支。
        then_value: Box<Expr>,
        /// 假分支。
        else_value: Box<Expr>,
    },
    /// 记录构造。
    Record {
        /// 各字段表达式。
        fields: BTreeMap<String, Expr>,
    },
    /// 列表构造。
    List {
        /// 元素类型。
        item_type: ValueType,
        /// 元素表达式。
        items: Vec<Expr>,
    },
    /// 封装可选值。
    Some {
        /// 内部值。
        value: Box<Expr>,
    },
    /// 调用固定纯函数。
    Function {
        /// 函数类型。
        function: Function,
        /// 有序参数。
        arguments: Vec<Expr>,
    },
}

impl Expr {
    /// 构造整数常量。
    pub fn int(value: i64) -> Self {
        Self::Literal {
            value_type: ValueType::Int,
            value: Value::Int(value),
        }
    }
    /// 构造文字常量。
    pub fn text(value: impl Into<String>) -> Self {
        Self::Literal {
            value_type: ValueType::Text,
            value: Value::Text(value.into()),
        }
    }
    /// 构造布尔常量。
    pub fn boolean(value: bool) -> Self {
        Self::Literal {
            value_type: ValueType::Bool,
            value: Value::Bool(value),
        }
    }
    /// 构造词法变量引用。
    pub fn var(name: impl Into<String>) -> Self {
        Self::Variable { name: name.into() }
    }
    /// 构造二元表达式。
    pub fn binary(op: BinaryOp, left: Self, right: Self) -> Self {
        Self::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
