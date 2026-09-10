//! 中英文前缀检索与 Monaco 插入契约；不重写查询语法。
use crate::{localization::chinese, localize};
use argusflow_aql::{EditorPosition, EditorRange, Span, SymbolKind, TokenKind, symbols, tokenize};
use serde::Serialize;
use std::collections::BTreeSet;

/// 候选项与中文草稿的替换范围。
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedCompletion {
    /// 显示的中文符号或原始参数名。
    pub label: String,
    /// 用于 Monaco 继续筛选的匹配词，可能是英文前缀。
    pub filter_text: String,
    /// 插入文本；角色和函数使用片段把光标放入括号。
    pub insert_text: String,
    /// 是否按 Monaco 片段语法解释插入文本。
    pub insert_as_snippet: bool,
    /// 中英文用法与类型简述。
    pub detail: String,
    /// 中文用途说明。
    pub description: String,
    /// 中文示例。
    pub example: String,
    /// 中文原稿的 UTF-16 替换范围。
    pub range: EditorRange,
    /// 候选项类别。
    pub kind: SymbolKind,
}
/// 在关键词前缀和 $参数前缀处补全；字符串、注释和正则中不补全。
pub fn suggest(source: &str, position: EditorPosition) -> Vec<LocalizedCompletion> {
    let offset = position.offset(source);
    let lexed = tokenize(source);
    if lexed.tokens.is_empty() && !lexed.diagnostics.is_empty() {
        return Vec::new();
    }
    let token = lexed
        .tokens
        .iter()
        .find(|token| token.span.start < offset && offset <= token.span.end);
    let parameter = token.is_some_and(|t| t.kind == TokenKind::Parameter || t.text(source) == "$");
    if !parameter
        && token.is_some_and(|t| {
            matches!(
                t.kind,
                TokenKind::String | TokenKind::Regex | TokenKind::Comment | TokenKind::Invalid
            )
        })
    {
        return Vec::new();
    }
    let span = token
        .filter(|t| t.kind == TokenKind::Identifier || parameter)
        .map(|t| t.span)
        .unwrap_or(Span::new(offset, offset));
    let prefix = &source[span.start..offset];
    if parameter {
        let names = lexed
            .tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Parameter && t.span != span)
            .map(|t| t.text(source))
            .collect::<BTreeSet<_>>();
        return names
            .into_iter()
            .filter(|name| name.starts_with(prefix))
            .map(|name| LocalizedCompletion {
                label: name.into(),
                filter_text: name.into(),
                insert_text: name.into(),
                insert_as_snippet: false,
                detail: "命名参数".into(),
                description: "引用当前查询中已有的参数；执行时由调用方绑定值。".into(),
                example: name.into(),
                range: span.editor_range(source),
                kind: SymbolKind::Parameter,
            })
            .collect();
    }
    let following_call = lexed
        .tokens
        .iter()
        .find(|t| {
            t.span.start >= span.end
                && !matches!(t.kind, TokenKind::Whitespace | TokenKind::Comment)
        })
        .is_some_and(|t| t.text(source) == "(");
    symbols()
        .into_iter()
        .filter_map(|symbol| {
            let label = chinese(&symbol.name);
            let filter_text = [
                label,
                symbol.name.as_str(),
                label.rsplit('.').next().unwrap_or(label),
                symbol.name.rsplit('.').next().unwrap_or(&symbol.name),
            ]
            .into_iter()
            .find(|candidate| candidate.starts_with(prefix))?
            .to_owned();
            let snippet =
                !following_call && matches!(symbol.kind, SymbolKind::Role | SymbolKind::Function);
            Some(LocalizedCompletion {
                label: label.into(),
                filter_text,
                insert_text: if snippet {
                    format!("{label}($0)")
                } else {
                    label.into()
                },
                insert_as_snippet: snippet,
                detail: format!("{} · {}", symbol.name, localize(&symbol.signature).source()),
                description: symbol.description,
                example: localize(&symbol.example).source().into(),
                range: span.editor_range(source),
                kind: symbol.kind,
            })
        })
        .collect()
}
