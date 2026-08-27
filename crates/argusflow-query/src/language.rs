use argusflow_core::UiQuery;
use serde::{Deserialize, Serialize};

use crate::{
    AqlError, AqlErrorKind, CompletionItem, CompletionItemKind, Diagnostic, DiagnosticCode,
    DiagnosticParams, DiagnosticSeverity, EditorPosition, EditorRange, Hover, RawToken,
    RawTokenKind, SyntaxToken, SyntaxTokenKind, SyntaxTree, TextEdit, analyze_query,
    build_recovery_tree, byte_range_to_editor_range, format_query, lex_lossless, parse_query,
};

/// 对完整或不完整源码的一次容错语言分析结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDocument {
    /// 保留 trivia、错误 token 与不完整节点的 CST。
    pub syntax: SyntaxTree,
    /// 所有可恢复的词法、结构与 HIR 诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// 只有语法和语义完整有效时才存在的 HIR。
    pub hir: Option<UiQuery>,
    /// 由 Rust 语言引擎分类的高亮 token。
    pub semantic_tokens: Vec<SyntaxToken>,
}

/// 由语言服务提供的格式化与 IDE 数据集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDocument {
    /// Recovery parser 的完整结果。
    pub parsed: ParsedDocument,
    /// 有效文档的纯排版结果。
    pub formatted_source: Option<String>,
    /// 有效文档的 canonical identity。
    pub canonical_source: Option<String>,
}

/// 对任意源码生成 lossless CST、多个诊断、可选 HIR 与语法高亮。
pub fn parse_document(source: &str) -> ParsedDocument {
    let (raw_tokens, lexical_diagnostics) = lex_lossless(source);
    let semantic_tokens = raw_tokens.iter().map(classify_token).collect::<Vec<_>>();
    let (syntax, mut diagnostics) = build_recovery_tree(raw_tokens, lexical_diagnostics);

    let hir = match parse_query(source) {
        Ok(query) if !diagnostics.iter().any(is_error) => Some(query),
        Ok(_) => None,
        Err(error) => {
            let diagnostic = diagnostic_from_error(source, &error);
            if !diagnostics.iter().any(|candidate| {
                candidate.code == diagnostic.code && candidate.range == diagnostic.range
            }) {
                diagnostics.push(diagnostic);
            }
            None
        }
    };
    diagnostics.sort_by_key(|diagnostic| {
        diagnostic.range.map_or((u32::MAX, u32::MAX), |range| {
            (range.start.line, range.start.utf16_column)
        })
    });

    ParsedDocument {
        syntax,
        diagnostics,
        hir,
        semantic_tokens,
    }
}

/// 生成编辑器一次输入所需的完整本地语言结果。
pub fn inspect_document(source: &str) -> LanguageDocument {
    let parsed = parse_document(source);
    let (formatted_source, canonical_source) = parsed.hir.as_ref().map_or((None, None), |query| {
        let analysis = analyze_query(query);
        (
            Some(format_query(query)),
            Some(analysis.canonical_source().to_owned()),
        )
    });
    LanguageDocument {
        parsed,
        formatted_source,
        canonical_source,
    }
}

/// 根据光标前的 token 语境返回 Rust grammar 生成的补全候选。
pub fn completions(source: &str, position: EditorPosition) -> Vec<CompletionItem> {
    let parsed = parse_document(source);
    let replacement_range = word_range_at_position(source, position);
    let prefix = text_in_range(source, replacement_range);
    let inside_arguments = unmatched_left_parentheses(&parsed.syntax.tokens) > 0;
    let candidates: &[(&str, CompletionItemKind, &str)] = if inside_arguments {
        &[
            ("name", CompletionItemKind::Property, "Accessible Name"),
            ("key", CompletionItemKind::Property, "跨后端逻辑键"),
            ("value", CompletionItemKind::Property, "元素当前值"),
            ("enabled", CompletionItemKind::Property, "是否可交互"),
            ("visible", CompletionItemKind::Property, "是否可见"),
            ("focused", CompletionItemKind::Property, "是否拥有焦点"),
            ("checked", CompletionItemKind::Property, "是否勾选"),
            ("selected", CompletionItemKind::Property, "是否选中"),
            (
                "uia.automation_id",
                CompletionItemKind::Property,
                "Windows UIA AutomationId",
            ),
            (
                "uia.class_name",
                CompletionItemKind::Property,
                "Windows UIA ClassName",
            ),
            (
                "uia.accelerator_key",
                CompletionItemKind::Property,
                "Windows UIA 命令快捷键",
            ),
            (
                "uia.access_key",
                CompletionItemKind::Property,
                "Windows UIA 助记键",
            ),
            (
                "uia.framework_id",
                CompletionItemKind::Property,
                "Windows UIA provider framework",
            ),
            (
                "dom.test_id",
                CompletionItemKind::Property,
                "DOM data-testid",
            ),
            ("contains", CompletionItemKind::Operator, "包含文本"),
            ("matches", CompletionItemKind::Operator, "正则匹配"),
            ("true", CompletionItemKind::Value, "布尔真值"),
            ("false", CompletionItemKind::Value, "布尔假值"),
        ]
    } else {
        &[
            ("button", CompletionItemKind::Role, "按钮"),
            ("textbox", CompletionItemKind::Role, "文本框"),
            ("window", CompletionItemKind::Role, "窗口"),
            ("dialog", CompletionItemKind::Role, "对话框"),
            ("any", CompletionItemKind::Function, "按顺序组合替代查询"),
            ("not", CompletionItemKind::Function, "排除查询结果"),
            ("first", CompletionItemKind::Function, "选择第一个结果"),
            ("nth", CompletionItemKind::Function, "选择第 N 个结果"),
            (
                "css",
                CompletionItemKind::Function,
                "CDP 原生 CSS escape hatch",
            ),
        ]
    };

    candidates
        .iter()
        .filter(|(label, _, _)| prefix.is_empty() || label.starts_with(prefix))
        .map(|(label, kind, detail)| CompletionItem {
            label: (*label).to_owned(),
            replacement_range,
            insert_text: if matches!(
                kind,
                CompletionItemKind::Role | CompletionItemKind::Function
            ) {
                format!("{label}()")
            } else {
                (*label).to_owned()
            },
            kind: *kind,
            detail: Some((*detail).to_owned()),
        })
        .collect()
}

/// 返回光标所在语法 token 的稳定 Hover 描述代码。
pub fn hover(source: &str, position: EditorPosition) -> Option<Hover> {
    let parsed = parse_document(source);
    let token = parsed.syntax.tokens.iter().find(|token| {
        position_at_or_after(position, token.range.start)
            && position_before(position, token.range.end)
            && token.kind != RawTokenKind::Whitespace
    })?;
    Some(Hover {
        range: token.range,
        symbol: token.text.clone(),
        description_code: format!("aql.hover.{}", hover_suffix(token)),
    })
}

/// 为常见错误提供由 Rust grammar 生成的安全文本修改。
pub fn code_actions(source: &str) -> Vec<TextEdit> {
    let Some(open_bracket) = source.find('[') else {
        return Vec::new();
    };
    let Some(close_bracket) = source.rfind(']') else {
        return Vec::new();
    };
    if close_bracket <= open_bracket {
        return Vec::new();
    }
    vec![
        TextEdit {
            range: byte_range_to_editor_range(source, open_bracket, open_bracket + 1),
            new_text: "(".to_owned(),
        },
        TextEdit {
            range: byte_range_to_editor_range(source, close_bracket, close_bracket + 1),
            new_text: ")".to_owned(),
        },
    ]
}

/// 把 fail-fast runtime error 转换为不携带产品文案的编辑器诊断。
fn diagnostic_from_error(source: &str, error: &AqlError) -> Diagnostic {
    let code = match error.kind {
        AqlErrorKind::EmptyQuery => DiagnosticCode::EmptyQuery,
        AqlErrorKind::InvalidToken => DiagnosticCode::InvalidToken,
        AqlErrorKind::UnexpectedToken => DiagnosticCode::UnexpectedToken,
        AqlErrorKind::UnknownRole => DiagnosticCode::UnknownRole,
        AqlErrorKind::UnknownProperty => DiagnosticCode::UnknownProperty,
        AqlErrorKind::UnknownOperator => DiagnosticCode::UnknownOperator,
        AqlErrorKind::InvalidPredicate => DiagnosticCode::InvalidPredicate,
        AqlErrorKind::InvalidRegex => DiagnosticCode::InvalidRegex,
        AqlErrorKind::InvalidArgument => DiagnosticCode::InvalidArgument,
        AqlErrorKind::CssSyntax => DiagnosticCode::CssSyntax,
    };
    Diagnostic {
        code,
        severity: DiagnosticSeverity::Error,
        range: Some(byte_range_to_editor_range(
            source,
            error.span.start,
            error.span.end,
        )),
        backend: None,
        params: DiagnosticParams::None,
    }
}

/// 把 lossless token 分类为语义高亮类别。
fn classify_token(token: &RawToken) -> SyntaxToken {
    let kind = match token.kind {
        RawTokenKind::Whitespace => SyntaxTokenKind::Trivia,
        RawTokenKind::String => SyntaxTokenKind::String,
        RawTokenKind::Regex => SyntaxTokenKind::Regex,
        RawTokenKind::Integer => SyntaxTokenKind::Integer,
        RawTokenKind::Boolean => SyntaxTokenKind::Boolean,
        RawTokenKind::LeftParen | RawTokenKind::RightParen | RawTokenKind::Comma => {
            SyntaxTokenKind::Punctuation
        }
        RawTokenKind::Operator => SyntaxTokenKind::Operator,
        RawTokenKind::Error if token.text.starts_with('"') => SyntaxTokenKind::String,
        RawTokenKind::Error if token.text.starts_with('/') => SyntaxTokenKind::Regex,
        RawTokenKind::Error => SyntaxTokenKind::Unknown,
        RawTokenKind::Identifier => classify_identifier(&token.text),
    };
    SyntaxToken {
        kind,
        range: token.range,
    }
}

/// 使用唯一 Rust 语言实现对标识符做高亮分类。
fn classify_identifier(identifier: &str) -> SyntaxTokenKind {
    if matches!(identifier, "any" | "not" | "first" | "nth" | "css") {
        SyntaxTokenKind::Function
    } else if matches!(
        identifier,
        "name" | "key" | "value" | "enabled" | "visible" | "focused" | "checked" | "selected"
    ) {
        SyntaxTokenKind::Property
    } else if identifier.starts_with("uia.") || identifier.starts_with("dom.") {
        SyntaxTokenKind::Namespace
    } else if matches!(
        identifier,
        "contains" | "starts_with" | "ends_with" | "matches"
    ) {
        SyntaxTokenKind::Operator
    } else if is_role(identifier) {
        SyntaxTokenKind::Role
    } else {
        SyntaxTokenKind::Unknown
    }
}

/// 判断标识符是否属于 AQL v1 角色清单。
fn is_role(identifier: &str) -> bool {
    matches!(
        identifier,
        "window"
            | "dialog"
            | "pane"
            | "button"
            | "textbox"
            | "checkbox"
            | "radio"
            | "combobox"
            | "list"
            | "list_item"
            | "tree"
            | "tree_item"
            | "tab"
            | "tab_item"
            | "menu"
            | "menu_item"
            | "link"
            | "image"
            | "table"
            | "row"
            | "cell"
            | "document"
            | "text"
    )
}

/// 返回尚未闭合的左括号数量。
fn unmatched_left_parentheses(tokens: &[RawToken]) -> usize {
    tokens
        .iter()
        .fold(0_usize, |depth, token| match token.kind {
            RawTokenKind::LeftParen => depth + 1,
            RawTokenKind::RightParen => depth.saturating_sub(1),
            _ => depth,
        })
}

/// 计算光标位置所在 ASCII 标识符的替换范围。
fn word_range_at_position(source: &str, position: EditorPosition) -> EditorRange {
    let offset = editor_position_to_byte_offset(source, position);
    let mut start = offset;
    while start > 0 {
        let Some(character) = source[..start].chars().next_back() else {
            break;
        };
        if !(character.is_ascii_alphanumeric() || matches!(character, '_' | '.')) {
            break;
        }
        start -= character.len_utf8();
    }
    byte_range_to_editor_range(source, start, offset)
}

/// 读取 UTF-16 范围内的源码；补全范围只会落在 ASCII 标识符中。
fn text_in_range(source: &str, range: EditorRange) -> &str {
    let start = editor_position_to_byte_offset(source, range.start);
    let end = editor_position_to_byte_offset(source, range.end);
    &source[start..end]
}

/// 将 WebView 位置转换回 Rust 字节偏移。
fn editor_position_to_byte_offset(source: &str, position: EditorPosition) -> usize {
    let mut line = 0_u32;
    let mut column = 0_u32;
    for (offset, character) in source.char_indices() {
        if line == position.line && column >= position.utf16_column {
            return offset;
        }
        if character == '\n' {
            if line == position.line {
                return offset;
            }
            line += 1;
            column = 0;
        } else if line == position.line {
            column += character.len_utf16() as u32;
        }
    }
    source.len()
}

/// 判断诊断是否阻止 HIR。
fn is_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == DiagnosticSeverity::Error
}

/// 行列位置的全序大于等于比较。
fn position_at_or_after(left: EditorPosition, right: EditorPosition) -> bool {
    (left.line, left.utf16_column) >= (right.line, right.utf16_column)
}

/// 行列位置的全序小于比较。
fn position_before(left: EditorPosition, right: EditorPosition) -> bool {
    (left.line, left.utf16_column) < (right.line, right.utf16_column)
}

/// 把 token 类别映射为 Hover 本地化键后缀。
fn hover_suffix(token: &RawToken) -> &'static str {
    match classify_identifier(&token.text) {
        SyntaxTokenKind::Role => "role",
        SyntaxTokenKind::Function => "function",
        SyntaxTokenKind::Property => "property",
        SyntaxTokenKind::Namespace => "backend_property",
        SyntaxTokenKind::Operator => "operator",
        _ => "literal",
    }
}
