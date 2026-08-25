use serde::{Deserialize, Serialize};

use crate::{EditorRange, QueryBackend};

/// 语言服务与后端编译器共享的稳定诊断代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    /// 文档为空。
    EmptyQuery,
    /// 存在无法识别或未结束的 token。
    InvalidToken,
    /// grammar 在当前位置无法继续。
    UnexpectedToken,
    /// 元素角色不受支持。
    UnknownRole,
    /// 属性不受支持。
    UnknownProperty,
    /// 运算符不受支持。
    UnknownOperator,
    /// 谓词类型组合无效。
    InvalidPredicate,
    /// 正则字面量无效。
    InvalidRegex,
    /// 函数参数无效。
    InvalidArgument,
    /// 使用了 CSS attribute selector 语法。
    CssSyntax,
    /// 缺少右括号。
    MissingRightParenthesis,
    /// 存在多余的右括号。
    UnexpectedRightParenthesis,
    /// 查询显式依赖后端专用能力。
    BackendSpecificProperty,
    /// 后端计划需要 residual filter。
    ResidualFilter,
    /// 后端计划需要额外遍历或多分支执行。
    ExpensiveTraversal,
    /// 查询可能产生多个目标。
    PotentialMultiMatch,
    /// 后端无法保持完整查询语义。
    UnsupportedBackend,
    /// 后端执行器或运行上下文不可用。
    RuntimeUnavailable,
}

/// 诊断严重程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// 阻止 HIR lowering 或执行计划生成。
    Error,
    /// 查询有效，但存在性能、可移植性或运行时问题。
    Warning,
    /// 非阻断的解释信息。
    Information,
}

/// 诊断代码所需的结构化参数；产品文案不由 Rust 拼接。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiagnosticParams {
    /// 无额外参数。
    None,
    /// 诊断与一个源码 token 有关。
    Token {
        /// 原始 token 文本。
        token: String,
    },
    /// 诊断说明缺少某个语法元素。
    Expected {
        /// 稳定的期望元素名称。
        expected: String,
    },
    /// 诊断与最低数量约束有关。
    MinimumCount {
        /// 允许的最小数量。
        minimum: usize,
    },
}

/// 不绑定产品语言的结构化 AQL 诊断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 稳定机器码。
    pub code: DiagnosticCode,
    /// 错误、警告或说明。
    pub severity: DiagnosticSeverity,
    /// 可定位诊断才携带编辑器范围。
    pub range: Option<EditorRange>,
    /// 仅影响单个后端时记录其类别。
    pub backend: Option<QueryBackend>,
    /// 本地化文案所需的结构化参数。
    pub params: DiagnosticParams,
}

impl Diagnostic {
    /// 创建不绑定源码范围的后端或语义诊断。
    pub const fn global(
        code: DiagnosticCode,
        severity: DiagnosticSeverity,
        backend: Option<QueryBackend>,
    ) -> Self {
        Self {
            code,
            severity,
            range: None,
            backend,
            params: DiagnosticParams::None,
        }
    }
}
