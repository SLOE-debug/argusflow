//! 类型化选择器代数；动作不属于查询表达式。
use super::{Attribute, Role, Value};
use std::num::NonZeroUsize;

/// 结构查询不会隐式穿越的文档边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// iframe 文档，包括跨进程 frame。
    Frame,
    /// 开放 Shadow Root。
    Shadow,
}
/// 普通元素树上的范围关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// 直接子节点。
    Child,
    /// 任意深度后代，不含自身。
    Descendant,
}
/// 受类型检查约束的比较操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOperator {
    /// 精确相等。
    Equal,
    /// 不相等。
    NotEqual,
    /// 子串。
    Contains,
    /// 前缀。
    StartsWith,
    /// 后缀。
    EndsWith,
    /// Rust 正则。
    Regex,
    /// 数值大于。
    Greater,
    /// 数值大于等于。
    GreaterEqual,
    /// 数值小于。
    Less,
    /// 数值小于等于。
    LessEqual,
}
impl MatchOperator {
    /// 规范英文拼写。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::Contains => "contains",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
            Self::Regex => "matches",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Less => "<",
            Self::LessEqual => "<=",
        }
    }
}
/// 绑定前后的右值。
#[derive(Debug, Clone)]
pub enum Operand {
    /// 文字、布尔或数字。
    Literal(Value),
    /// 类型由属性推断的参数。
    Parameter(String),
    /// 编译时验证的正则及大小写标记。
    Regex {
        /// 未加分隔符的模式。
        pattern: String,
        /// 是否忽略大小写。
        insensitive: bool,
        /// 复用已编译模式，全部后端使用同一引擎。
        compiled: regex::Regex,
    },
}
/// 属性级布尔表达式；缺失属性在取反时仍为未知。
#[derive(Debug, Clone)]
pub enum Condition {
    /// 属性比较。
    Compare {
        /// 左属性。
        attribute: Attribute,
        /// 比较操作。
        operator: MatchOperator,
        /// 右值。
        operand: Operand,
    },
    /// 两侧均成立。
    And(Box<Self>, Box<Self>),
    /// 任意一侧成立。
    Or(Box<Self>, Box<Self>),
    /// 布尔取反。
    Not(Box<Self>),
}
/// 已通过语义检查的选择器结构。
#[derive(Debug, Clone)]
pub enum Expr {
    /// 角色与可选条件。
    Match {
        /// 角色；Element 表示所有真实元素。
        role: Role,
        /// 没有条件时匹配该角色全部元素。
        condition: Option<Condition>,
    },
    /// 限定右侧查询的结构范围。
    Relation {
        /// 范围来源。
        left: Box<Self>,
        /// 范围中的目标。
        right: Box<Self>,
        /// 直接子级或后代。
        relation: Relation,
    },
    /// 明确选第几个结果。
    Nth {
        /// 被选择的查询。
        query: Box<Self>,
        /// 从一开始。
        index: NonZeroUsize,
    },
    /// 浏览器原生 CSS。
    Css(String),
    /// 显式进入由唯一宿主定位的边界。
    Enter {
        /// iframe 元素或 Shadow 宿主。
        host: Box<Self>,
        /// 进入的文档边界。
        boundary: Boundary,
    },
}
