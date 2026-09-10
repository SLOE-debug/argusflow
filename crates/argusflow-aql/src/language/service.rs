//! 即使源码无效也返回 token 和定位诊断。
use super::{SymbolKind, format_query, symbols};
use crate::{AqlError, EditorPosition, Span, Token, TokenKind, compile, tokenize};
use serde::{Deserialize, Serialize};

/// 编辑器分析结果不携带后端资源。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    /// 无损词法范围。
    pub tokens: Vec<Token>,
    /// 当前文本的诊断。
    pub diagnostics: Vec<AqlError>,
    /// 仅有效源码有格式化结果。
    pub formatted: Option<String>,
}
/// 带替换范围的补全。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    /// 符号名称。
    pub label: String,
    /// 插入文本。
    pub insert_text: String,
    /// 源码字节范围。
    pub span: Span,
    /// 补全类别。
    pub kind: SymbolKind,
}
/// 分析当前英文源码，失败时不返回旧的有效结果。
pub fn analyze(source: &str) -> Analysis {
    let lexed = tokenize(source);
    let mut diagnostics = lexed.diagnostics;
    if diagnostics.is_empty()
        && let Err(error) = compile(source)
    {
        diagnostics.push(error);
    }
    let formatted = if diagnostics.is_empty() {
        format_query(source).ok()
    } else {
        None
    };
    Analysis {
        tokens: lexed.tokens,
        diagnostics,
        formatted,
    }
}
/// 为当前 token 提供语法词汇；在字符串、参数、正则和注释内部不补全关键字。
pub fn completions(source: &str, position: EditorPosition) -> Vec<Completion> {
    let offset = position.offset(source);
    let lexed = tokenize(source);
    let token = lexed
        .tokens
        .iter()
        .find(|t| t.span.start < offset && offset <= t.span.end);
    if token.is_some_and(|t| {
        matches!(
            t.kind,
            TokenKind::String
                | TokenKind::Regex
                | TokenKind::Parameter
                | TokenKind::Comment
                | TokenKind::Invalid
        )
    }) {
        return Vec::new();
    }
    let span = token
        .filter(|t| t.kind == TokenKind::Identifier)
        .map(|t| t.span)
        .unwrap_or(Span::new(offset, offset));
    let prefix = &source[span.start..offset];
    symbols()
        .into_iter()
        .filter(|s| s.name.starts_with(prefix))
        .map(|s| Completion {
            insert_text: if matches!(s.kind, SymbolKind::Role | SymbolKind::Function) {
                format!("{}()", s.name)
            } else {
                s.name.clone()
            },
            label: s.name,
            span,
            kind: s.kind,
        })
        .collect()
}
