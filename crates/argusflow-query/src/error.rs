use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AQL 源码中的半开字节区间及其人类可读起始位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    /// 起始 UTF-8 字节偏移。
    pub start: usize,
    /// 结束 UTF-8 字节偏移，不包含该位置。
    pub end: usize,
    /// 从一开始计数的行号。
    pub line: usize,
    /// 从一开始计数的 Unicode 字符列号。
    pub column: usize,
}

/// 稳定的 AQL 诊断类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AqlErrorKind {
    /// 输入不包含查询。
    EmptyQuery,
    /// 词法分析遇到无法识别的字符或未结束字面量。
    InvalidToken,
    /// 当前位置的语法结构与 grammar 不符。
    UnexpectedToken,
    /// 使用了 AQL v3 不支持的元素角色。
    UnknownRole,
    /// 使用了 AQL v3 不支持的属性。
    UnknownProperty,
    /// 使用了未知或 CSS 风格运算符。
    UnknownOperator,
    /// 属性、运算符和右值的类型组合无效。
    InvalidPredicate,
    /// 正则表达式无法由 AQL v3 正则引擎编译。
    InvalidRegex,
    /// 查询组合器的参数数量或取值无效。
    InvalidArgument,
    /// 输入使用了 CSS 风格 attribute selector。
    CssSyntax,
}

/// 带源码位置和修复建议的 AQL 解析诊断。
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error("{message}（第 {} 行，第 {} 列）", .span.line, .span.column)]
pub struct AqlError {
    /// 稳定的机器可读错误类别。
    pub kind: AqlErrorKind,
    /// 对应源码位置。
    pub span: SourceSpan,
    /// 面向用户的具体错误说明。
    pub message: String,
    /// 可选的 AQL 正确写法提示。
    pub help: Option<String>,
}

impl AqlError {
    /// 根据 UTF-8 字节区间构造包含行列信息的诊断。
    pub(crate) fn at(
        source: &str,
        start: usize,
        end: usize,
        kind: AqlErrorKind,
        message: impl Into<String>,
        help: Option<String>,
    ) -> Self {
        // 只统计错误前缀，避免字节偏移落在多字节字符之后时列号失真。
        let prefix = &source[..start.min(source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit_once('\n')
            .map_or(prefix, |(_, current_line)| current_line)
            .chars()
            .count()
            + 1;

        Self {
            kind,
            span: SourceSpan {
                start,
                end,
                line,
                column,
            },
            message: message.into(),
            help,
        }
    }
}
