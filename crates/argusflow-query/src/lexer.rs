use crate::{AqlError, AqlErrorKind, DiagnosticCode, RawToken, RawTokenKind, lex_lossless};

/// HIR parser 内部使用的已解码 token。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    /// token 的语义类别与已解码字面量。
    pub(crate) kind: TokenKind,
    /// 起始 UTF-8 字节偏移。
    pub(crate) start: usize,
    /// 结束 UTF-8 字节偏移。
    pub(crate) end: usize,
}

/// AQL v1 的语义词法单元集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// 角色、属性、操作符关键字或组合器名称。
    Identifier(String),
    /// 已移除 `$` 前缀的运行时参数名。
    Parameter(String),
    /// 已按 JSON 字符串转义规则解码的文本。
    String(String),
    /// 已移除分隔符并解析 `i` 标志的正则字面量。
    Regex {
        /// 正则模式。
        pattern: String,
        /// 是否忽略大小写。
        case_insensitive: bool,
    },
    /// `nth` 使用的非负十进制整数。
    Integer(usize),
    /// 布尔真值。
    True,
    /// 布尔假值。
    False,
    /// `(`。
    LeftParen,
    /// `)`。
    RightParen,
    /// `,`。
    Comma,
    /// `=`。
    Equal,
    /// `!=`。
    NotEqual,
    /// `>`。
    Child,
    /// `>>`。
    Descendant,
    /// 输入结尾。
    End,
}

/// 复用 lossless lexer 的唯一 token 边界，并解码 HIR parser 所需字面量。
pub(crate) fn lex(source: &str) -> Result<Vec<Token>, AqlError> {
    let (raw_tokens, diagnostics) = lex_lossless(source);
    if let Some(diagnostic) = diagnostics.first() {
        let raw_token = raw_tokens
            .iter()
            .find(|token| diagnostic.range == Some(token.range));
        return Err(error_from_diagnostic(source, diagnostic.code, raw_token));
    }

    let mut tokens = raw_tokens
        .iter()
        .filter(|token| token.kind != RawTokenKind::Whitespace)
        .map(|token| lower_token(source, token))
        .collect::<Result<Vec<_>, _>>()?;
    tokens.push(Token {
        kind: TokenKind::End,
        start: source.len(),
        end: source.len(),
    });
    Ok(tokens)
}

/// 将一个 lossless token 解码为语义 parser token。
fn lower_token(source: &str, token: &RawToken) -> Result<Token, AqlError> {
    let kind = match token.kind {
        RawTokenKind::Identifier => TokenKind::Identifier(token.text.clone()),
        RawTokenKind::Parameter => TokenKind::Parameter(token.text[1..].to_owned()),
        RawTokenKind::String => {
            TokenKind::String(serde_json::from_str(&token.text).map_err(|error| {
                AqlError::at(
                    source,
                    token.byte_start,
                    token.byte_end,
                    AqlErrorKind::InvalidToken,
                    format!("字符串字面量无效：{error}"),
                    Some("AQL 字符串使用 JSON 双引号与转义规则".to_owned()),
                )
            })?)
        }
        RawTokenKind::Regex => lower_regex(source, token)?,
        RawTokenKind::Integer => TokenKind::Integer(token.text.parse().map_err(|_| {
            AqlError::at(
                source,
                token.byte_start,
                token.byte_end,
                AqlErrorKind::InvalidArgument,
                "nth 索引超出可表示范围",
                None,
            )
        })?),
        RawTokenKind::Boolean if token.text == "true" => TokenKind::True,
        RawTokenKind::Boolean => TokenKind::False,
        RawTokenKind::LeftParen => TokenKind::LeftParen,
        RawTokenKind::RightParen => TokenKind::RightParen,
        RawTokenKind::Comma => TokenKind::Comma,
        RawTokenKind::Operator => match token.text.as_str() {
            "=" => TokenKind::Equal,
            "!=" => TokenKind::NotEqual,
            ">" => TokenKind::Child,
            ">>" => TokenKind::Descendant,
            operator => {
                return Err(AqlError::at(
                    source,
                    token.byte_start,
                    token.byte_end,
                    AqlErrorKind::UnknownOperator,
                    format!("未知 AQL 运算符 '{operator}'"),
                    None,
                ));
            }
        },
        RawTokenKind::Error | RawTokenKind::Whitespace => {
            return Err(error_from_diagnostic(
                source,
                DiagnosticCode::InvalidToken,
                Some(token),
            ));
        }
    };
    Ok(Token {
        kind,
        start: token.byte_start,
        end: token.byte_end,
    })
}

/// 解码 `/pattern/i`，只移除用于转义分隔符的反斜线。
fn lower_regex(source: &str, token: &RawToken) -> Result<TokenKind, AqlError> {
    let Some(closing_offset) = find_regex_closing_delimiter(&token.text) else {
        return Err(error_from_diagnostic(
            source,
            DiagnosticCode::InvalidToken,
            Some(token),
        ));
    };
    let raw_pattern = &token.text[1..closing_offset];
    let flags = &token.text[closing_offset + 1..];
    let mut pattern = String::new();
    let mut characters = raw_pattern.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' && characters.peek() == Some(&'/') {
            pattern.push(characters.next().expect("peek proved escaped slash exists"));
        } else {
            pattern.push(character);
        }
    }
    Ok(TokenKind::Regex {
        pattern,
        case_insensitive: flags == "i",
    })
}

/// 找到未转义的正则结束分隔符。
fn find_regex_closing_delimiter(literal: &str) -> Option<usize> {
    let mut escaped = false;
    for (offset, character) in literal.char_indices().skip(1) {
        if character == '/' && !escaped {
            return Some(offset);
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    None
}

/// 将 lossless 诊断映射为保留原 Runtime API 的 fail-fast 错误。
fn error_from_diagnostic(source: &str, code: DiagnosticCode, token: Option<&RawToken>) -> AqlError {
    let (start, end, text) = token.map_or((0, 0, ""), |token| {
        (token.byte_start, token.byte_end, token.text.as_str())
    });
    let (kind, message, help) = match code {
        DiagnosticCode::CssSyntax => (
            AqlErrorKind::CssSyntax,
            "AQL 不支持 CSS 风格的属性选择器".to_owned(),
            Some(
                "请使用 button(name = \"保存\")；原生 CSS 请写成 css(\"button[name='保存']\")"
                    .to_owned(),
            ),
        ),
        DiagnosticCode::UnknownOperator => (
            AqlErrorKind::UnknownOperator,
            format!("未知或不完整的 AQL 运算符 '{text}'"),
            Some("不等比较使用 '!='；查询取反使用 not(...)".to_owned()),
        ),
        DiagnosticCode::InvalidRegex => (
            AqlErrorKind::InvalidRegex,
            "AQL v1 正则标志无效".to_owned(),
            Some("AQL v1 仅支持可选的 i 标志".to_owned()),
        ),
        _ => (
            AqlErrorKind::InvalidToken,
            "AQL 包含无法识别或未结束的 token".to_owned(),
            None,
        ),
    };
    AqlError::at(source, start, end, kind, message, help)
}
