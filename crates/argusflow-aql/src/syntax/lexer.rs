//! 无损范围词法器；未知和不完整输入保留 token，供编辑器继续编辑。
use crate::{AqlError, DiagnosticCode, Span};
use serde::{Deserialize, Serialize};

/// 词法类别不依赖关键字语言；严格英文检查由 parser 完成。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    /// 普通标识符。
    Identifier,
    /// 含引号的字符串。
    String,
    /// 含分隔符和可选 i 的正则。
    Regex,
    /// 含 $ 前缀的参数。
    Parameter,
    /// 数值字面量。
    Number,
    /// 空白。
    Whitespace,
    /// 行注释或块注释。
    Comment,
    /// 运算符、括号和逗号。
    Punctuation,
    /// 不完整或非法 token。
    Invalid,
}
/// 原始 token 的种类和源码范围。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// token 类别。
    pub kind: TokenKind,
    /// UTF-8 范围。
    pub span: Span,
}
impl Token {
    /// 从对应源码读取原始文本。
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.span.start..self.span.end]
    }
}
/// 包含空白和注释的词法结果，即使输入不完整也可使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lexed {
    /// 按源码顺序、不重叠的 token。
    pub tokens: Vec<Token>,
    /// 词法失败列表。
    pub diagnostics: Vec<AqlError>,
}
/// 分词而不进行语言关键字翻译。源码上限 64 KiB。
pub fn tokenize(source: &str) -> Lexed {
    let mut result = Lexed {
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    if source.len() > 65_536 {
        result.diagnostics.push(AqlError::new(
            DiagnosticCode::Limit,
            Span::new(0, source.len()),
            "AQL 源码不能超过 64 KiB",
        ));
        return result;
    }
    let mut position = 0;
    while position < source.len() {
        let start = position;
        let ch = source[position..].chars().next().unwrap_or('\0');
        let kind;
        if ch.is_whitespace() {
            position += ch.len_utf8();
            while let Some(next) = source[position..]
                .chars()
                .next()
                .filter(|c| c.is_whitespace())
            {
                position += next.len_utf8();
            }
            kind = TokenKind::Whitespace;
        } else if source[position..].starts_with("//") {
            position += source[position..]
                .find('\n')
                .unwrap_or(source.len() - position);
            kind = TokenKind::Comment;
        } else if source[position..].starts_with("/*") {
            if let Some(end) = source[position + 2..].find("*/") {
                position += end + 4;
                kind = TokenKind::Comment;
            } else {
                position = source.len();
                kind = TokenKind::Invalid;
            }
        } else if ch == '"' || ch == '/' {
            let delimiter = ch;
            position += 1;
            let mut escaped = false;
            let mut closed = false;
            let mut in_class = false;
            while let Some(next) = source[position..].chars().next() {
                position += next.len_utf8();
                if escaped {
                    escaped = false;
                    continue;
                }
                if next == '\\' {
                    escaped = true;
                    continue;
                }
                if delimiter == '/' {
                    if next == '[' {
                        in_class = true;
                    }
                    if next == ']' {
                        in_class = false;
                    }
                }
                if next == delimiter && !in_class {
                    closed = true;
                    break;
                }
                if next == '\n' || next == '\r' {
                    break;
                }
            }
            if closed && delimiter == '/' {
                while let Some(flag) = source[position..]
                    .chars()
                    .next()
                    .filter(|c| c.is_ascii_alphabetic())
                {
                    position += flag.len_utf8();
                }
            }
            kind = if !closed {
                TokenKind::Invalid
            } else if delimiter == '"' {
                TokenKind::String
            } else {
                TokenKind::Regex
            };
        } else if ch == '$' || ch == '_' || ch.is_alphabetic() {
            position += ch.len_utf8();
            while let Some(next) = source[position..]
                .chars()
                .next()
                .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '.'))
            {
                position += next.len_utf8();
            }
            kind = if ch == '$' {
                if position == start + 1 {
                    TokenKind::Invalid
                } else {
                    TokenKind::Parameter
                }
            } else {
                TokenKind::Identifier
            };
        } else if ch.is_ascii_digit()
            || (ch == '-' && source[position + 1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            position += ch.len_utf8();
            while let Some(next) = source[position..]
                .chars()
                .next()
                .filter(|c| c.is_ascii_digit() || *c == '.')
            {
                position += next.len_utf8();
            }
            kind = TokenKind::Number;
        } else {
            position += ch.len_utf8();
            if (matches!(ch, '>' | '<' | '!') && source[position..].starts_with('='))
                || (ch == '>' && source[position..].starts_with('>'))
            {
                position += 1;
            }
            kind = if matches!(ch, '(' | ')' | ',' | '=' | '!' | '>' | '<') {
                TokenKind::Punctuation
            } else {
                TokenKind::Invalid
            };
        }
        let span = Span::new(start, position);
        if kind == TokenKind::Invalid {
            result.diagnostics.push(AqlError::new(
                DiagnosticCode::Lexical,
                span,
                "无法识别或尚未完成的字符、字符串、注释或正则",
            ));
        }
        result.tokens.push(Token { kind, span });
    }
    result
}
