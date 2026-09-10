//! 源码字节范围和编辑器 UTF-16 范围的显式转换。
use serde::{Deserialize, Serialize};

/// 半开 UTF-8 字节范围。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// 起始字节。
    pub start: usize,
    /// 结束字节，不包含。
    pub end: usize,
}
impl Span {
    /// 构造范围；用于已知词法边界。
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    /// 转换到零基行和 UTF-16 列。
    pub fn editor_range(self, source: &str) -> EditorRange {
        EditorRange {
            start: EditorPosition::at(source, self.start),
            end: EditorPosition::at(source, self.end),
        }
    }
}
/// 零基编辑器位置；列使用 UTF-16 code unit。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorPosition {
    /// 零基行。
    pub line: u32,
    /// 零基 UTF-16 列。
    pub column: u32,
}
impl EditorPosition {
    /// 将字节偏移向前夹到字符边界。
    pub fn at(source: &str, offset: usize) -> Self {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() as u32;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count() as u32;
        Self { line, column }
    }
    /// 转换为字节偏移；超出范围的位置夹到行尾或 EOF。
    pub fn offset(self, source: &str) -> usize {
        let mut line = 0;
        let mut column = 0;
        for (offset, ch) in source.char_indices() {
            if line == self.line && (column >= self.column || ch == '\n') {
                return offset;
            }
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += ch.len_utf16() as u32;
            }
        }
        source.len()
    }
}
/// 半开编辑器范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorRange {
    /// 范围起点。
    pub start: EditorPosition,
    /// 范围终点。
    pub end: EditorPosition,
}
/// 编译和查询失败的稳定分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCode {
    /// 词法错误。
    Lexical,
    /// 语法错误。
    Syntax,
    /// 未知关键字。
    UnknownSymbol,
    /// 类型不一致。
    Type,
    /// 参数缺失或多余。
    Binding,
    /// 无效正则。
    Regex,
    /// 深度、节点或源码预算。
    Limit,
    /// 来源不支持该查询。
    Unsupported,
    /// 要求唯一但未找到。
    NotFound,
    /// 要求唯一但匹配多个。
    Ambiguous,
    /// 后端树不满足契约。
    InvalidTree,
}
/// 带源码位置的编译错误；消息不包含参数值。
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[error("{message}")]
pub struct AqlError {
    /// 稳定错误码。
    pub code: DiagnosticCode,
    /// 相关源码范围。
    pub span: Span,
    /// 可直接呈现的中文说明。
    pub message: String,
}
impl AqlError {
    /// 通过稳定分类和位置创建诊断。
    pub fn new(code: DiagnosticCode, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            message: message.into(),
        }
    }
    pub(crate) fn general(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self::new(code, Span::default(), message)
    }
}
