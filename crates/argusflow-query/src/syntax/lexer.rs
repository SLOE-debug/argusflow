use crate::{
    Diagnostic, DiagnosticCode, DiagnosticParams, DiagnosticSeverity, byte_range_to_editor_range,
};

use super::{RawToken, RawTokenKind};

/// 对任意 AQL 文本执行不丢 token 的容错词法分析。
pub(crate) fn lex_lossless(source: &str) -> (Vec<RawToken>, Vec<Diagnostic>) {
    let mut cursor = Cursor { source, offset: 0 };
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();

    while let Some(character) = cursor.peek() {
        let start = cursor.offset;
        let (kind, diagnostic) = match character {
            value if value.is_whitespace() => {
                cursor.take_while(char::is_whitespace);
                (RawTokenKind::Whitespace, None)
            }
            value if is_identifier_start(value) => {
                cursor.take_while(is_identifier_continue);
                let text = &source[start..cursor.offset];
                let kind = if matches!(text, "true" | "false") {
                    RawTokenKind::Boolean
                } else {
                    RawTokenKind::Identifier
                };
                (kind, None)
            }
            value if value.is_ascii_digit() => {
                cursor.take_while(|value| value.is_ascii_digit());
                (RawTokenKind::Integer, None)
            }
            '$' => {
                cursor.bump();
                if cursor.peek().is_some_and(is_identifier_start) {
                    cursor.bump();
                    cursor.take_while(is_identifier_continue);
                    (RawTokenKind::Parameter, None)
                } else {
                    (
                        RawTokenKind::Error,
                        Some((
                            DiagnosticCode::InvalidToken,
                            DiagnosticParams::Expected {
                                expected: "$identifier".to_owned(),
                            },
                        )),
                    )
                }
            }
            '"' => lex_quoted(&mut cursor, '"', RawTokenKind::String),
            '/' => lex_regex(&mut cursor),
            '(' => single(&mut cursor, RawTokenKind::LeftParen),
            ')' => single(&mut cursor, RawTokenKind::RightParen),
            ',' => single(&mut cursor, RawTokenKind::Comma),
            '[' => single(&mut cursor, RawTokenKind::LeftBracket),
            ']' => single(&mut cursor, RawTokenKind::RightBracket),
            '=' => single(&mut cursor, RawTokenKind::Operator),
            '!' => {
                cursor.bump();
                if cursor.peek() == Some('=') {
                    cursor.bump();
                    (RawTokenKind::Operator, None)
                } else {
                    (
                        RawTokenKind::Error,
                        Some((
                            DiagnosticCode::UnknownOperator,
                            DiagnosticParams::Token {
                                token: "!".to_owned(),
                            },
                        )),
                    )
                }
            }
            '>' => {
                cursor.bump();
                if matches!(cursor.peek(), Some('>' | '=')) {
                    cursor.bump();
                }
                (RawTokenKind::Operator, None)
            }
            '<' => {
                cursor.bump();
                if cursor.peek() == Some('=') {
                    cursor.bump();
                }
                (RawTokenKind::Operator, None)
            }
            '~' if cursor.source[cursor.offset..].starts_with("~=") => {
                cursor.bump();
                cursor.bump();
                (
                    RawTokenKind::Error,
                    Some((
                        DiagnosticCode::UnknownOperator,
                        DiagnosticParams::Token {
                            token: "~=".to_owned(),
                        },
                    )),
                )
            }
            _ => {
                cursor.bump();
                let text = source[start..cursor.offset].to_owned();
                let code = if character == '[' {
                    DiagnosticCode::CssSyntax
                } else {
                    DiagnosticCode::InvalidToken
                };
                (
                    RawTokenKind::Error,
                    Some((code, DiagnosticParams::Token { token: text })),
                )
            }
        };

        let end = cursor.offset;
        tokens.push(RawToken {
            kind,
            text: source[start..end].to_owned(),
            range: byte_range_to_editor_range(source, start, end),
            byte_start: start,
            byte_end: end,
        });
        if let Some((code, params)) = diagnostic {
            diagnostics.push(Diagnostic {
                code,
                severity: DiagnosticSeverity::Error,
                range: Some(byte_range_to_editor_range(source, start, end)),
                backend: None,
                params,
            });
        }
    }

    (tokens, diagnostics)
}

/// 消费固定宽度标点。
fn single(
    cursor: &mut Cursor<'_>,
    kind: RawTokenKind,
) -> (RawTokenKind, Option<(DiagnosticCode, DiagnosticParams)>) {
    cursor.bump();
    (kind, None)
}

/// 消费带反斜线转义的定界字面量，并将未闭合输入保留为错误 token。
fn lex_quoted(
    cursor: &mut Cursor<'_>,
    delimiter: char,
    valid_kind: RawTokenKind,
) -> (RawTokenKind, Option<(DiagnosticCode, DiagnosticParams)>) {
    cursor.bump();
    let mut escaped = false;
    while let Some(character) = cursor.bump() {
        if character == delimiter && !escaped {
            return (valid_kind, None);
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    (
        RawTokenKind::Error,
        Some((
            DiagnosticCode::InvalidToken,
            DiagnosticParams::Expected {
                expected: delimiter.to_string(),
            },
        )),
    )
}

/// 消费正则模式及其可选标志，并对未闭合或非法标志继续恢复。
fn lex_regex(
    cursor: &mut Cursor<'_>,
) -> (RawTokenKind, Option<(DiagnosticCode, DiagnosticParams)>) {
    let (kind, mut diagnostic) = lex_quoted(cursor, '/', RawTokenKind::Regex);
    if kind == RawTokenKind::Regex {
        let flags_start = cursor.offset;
        cursor.take_while(|value| value.is_ascii_alphabetic());
        let flags = &cursor.source[flags_start..cursor.offset];
        if !matches!(flags, "" | "i") {
            diagnostic = Some((
                DiagnosticCode::InvalidRegex,
                DiagnosticParams::Token {
                    token: flags.to_owned(),
                },
            ));
        }
    }
    (kind, diagnostic)
}

/// 维护 UTF-8 字节边界的 lossless lexer 游标。
struct Cursor<'source> {
    /// 完整源码。
    source: &'source str,
    /// 下一个字符的字节偏移。
    offset: usize,
}

impl Cursor<'_> {
    /// 查看下一个 Unicode scalar。
    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    /// 消费下一个 Unicode scalar。
    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    /// 连续消费满足谓词的字符。
    fn take_while(&mut self, predicate: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&predicate) {
            self.bump();
        }
    }
}

/// 标识符首字符规则。
const fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

/// 属性 namespace 允许点号，其合法性由 HIR lowering 校验。
const fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '.')
}
