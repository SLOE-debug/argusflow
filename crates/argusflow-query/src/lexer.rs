use crate::{AqlError, AqlErrorKind};

/// 解析器内部使用的带源码范围 token。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    /// token 的语义类别与已解码字面量。
    pub(crate) kind: TokenKind,
    /// 起始 UTF-8 字节偏移。
    pub(crate) start: usize,
    /// 结束 UTF-8 字节偏移。
    pub(crate) end: usize,
}

/// AQL v1 的最小词法单元集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// 角色、属性、操作符关键字或组合器名称。
    Identifier(String),
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

/// 将完整 AQL 源码转换为 token 序列。
pub(crate) fn lex(source: &str) -> Result<Vec<Token>, AqlError> {
    let mut lexer = Lexer { source, offset: 0 };
    let mut tokens = Vec::new();

    while let Some(character) = lexer.peek() {
        if character.is_whitespace() {
            lexer.bump();
            continue;
        }

        let start = lexer.offset;
        let kind = match character {
            '(' => {
                lexer.bump();
                TokenKind::LeftParen
            }
            ')' => {
                lexer.bump();
                TokenKind::RightParen
            }
            ',' => {
                lexer.bump();
                TokenKind::Comma
            }
            '=' => {
                lexer.bump();
                TokenKind::Equal
            }
            '!' => lex_not_equal(&mut lexer, start)?,
            '>' => lex_relation(&mut lexer),
            '"' => TokenKind::String(lex_string(&mut lexer, start)?),
            '/' => lex_regex(&mut lexer, start)?,
            '[' => {
                return Err(AqlError::at(
                    source,
                    start,
                    start + 1,
                    AqlErrorKind::CssSyntax,
                    "AQL 不支持 CSS 风格的属性选择器",
                    Some(
                        "请使用 button(name = \"保存\")；原生 CSS 请写成 css(\"button[name='保存']\")"
                            .to_owned(),
                    ),
                ));
            }
            '~' if lexer.remaining().starts_with("~=") => {
                lexer.bump();
                lexer.bump();
                return Err(AqlError::at(
                    source,
                    start,
                    lexer.offset,
                    AqlErrorKind::UnknownOperator,
                    "未知运算符 '~='；AQL 不使用 CSS 属性运算符",
                    Some("请使用 name matches /保存/ 或 name contains \"保存\"".to_owned()),
                ));
            }
            value if value.is_ascii_digit() => lex_integer(&mut lexer, start)?,
            value if is_identifier_start(value) => lex_identifier(&mut lexer),
            _ => {
                lexer.bump();
                return Err(AqlError::at(
                    source,
                    start,
                    lexer.offset,
                    AqlErrorKind::InvalidToken,
                    format!("AQL 中不允许字符 '{character}'"),
                    None,
                ));
            }
        };

        tokens.push(Token {
            kind,
            start,
            end: lexer.offset,
        });
    }

    tokens.push(Token {
        kind: TokenKind::End,
        start: source.len(),
        end: source.len(),
    });
    Ok(tokens)
}

/// 维护 UTF-8 字节偏移的源码游标。
struct Lexer<'source> {
    /// 完整 AQL 输入。
    source: &'source str,
    /// 下一个字符的 UTF-8 字节偏移。
    offset: usize,
}

impl Lexer<'_> {
    /// 查看但不消费下一个字符。
    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    /// 消费并返回下一个字符。
    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }

    /// 返回尚未消费的源码后缀。
    fn remaining(&self) -> &str {
        &self.source[self.offset..]
    }
}

/// 解析 `!=` 并为孤立的 `!` 提供精确错误。
fn lex_not_equal(lexer: &mut Lexer<'_>, start: usize) -> Result<TokenKind, AqlError> {
    lexer.bump();
    if lexer.peek() == Some('=') {
        lexer.bump();
        return Ok(TokenKind::NotEqual);
    }

    Err(AqlError::at(
        lexer.source,
        start,
        lexer.offset,
        AqlErrorKind::UnknownOperator,
        "'!' 不是完整的 AQL 运算符",
        Some("不等比较请使用 '!='；查询取反请使用 not(...)".to_owned()),
    ))
}

/// 区分直接子元素和后代关系。
fn lex_relation(lexer: &mut Lexer<'_>) -> TokenKind {
    lexer.bump();
    if lexer.peek() == Some('>') {
        lexer.bump();
        TokenKind::Descendant
    } else {
        TokenKind::Child
    }
}

/// 使用 JSON 字符串规则解码 AQL 文本字面量。
fn lex_string(lexer: &mut Lexer<'_>, start: usize) -> Result<String, AqlError> {
    lexer.bump();
    let mut escaped = false;

    while let Some(character) = lexer.bump() {
        if character == '"' && !escaped {
            let literal = &lexer.source[start..lexer.offset];
            return serde_json::from_str(literal).map_err(|error| {
                AqlError::at(
                    lexer.source,
                    start,
                    lexer.offset,
                    AqlErrorKind::InvalidToken,
                    format!("字符串字面量无效：{error}"),
                    Some("AQL 字符串使用 JSON 双引号与转义规则".to_owned()),
                )
            });
        }

        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }

    Err(AqlError::at(
        lexer.source,
        start,
        lexer.offset,
        AqlErrorKind::InvalidToken,
        "字符串字面量缺少结束双引号",
        None,
    ))
}

/// 解析 `/pattern/i`，只接受 v1 规定的可选 `i` 标志。
fn lex_regex(lexer: &mut Lexer<'_>, start: usize) -> Result<TokenKind, AqlError> {
    lexer.bump();
    let mut pattern = String::new();
    let mut escaped = false;

    while let Some(character) = lexer.bump() {
        if escaped {
            if character != '/' {
                pattern.push('\\');
            }
            pattern.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '/' {
            let flags_start = lexer.offset;
            while lexer.peek().is_some_and(|flag| flag.is_ascii_alphabetic()) {
                lexer.bump();
            }
            let flags = &lexer.source[flags_start..lexer.offset];
            if flags.is_empty() || flags == "i" {
                return Ok(TokenKind::Regex {
                    pattern,
                    case_insensitive: flags == "i",
                });
            }
            return Err(AqlError::at(
                lexer.source,
                flags_start,
                lexer.offset,
                AqlErrorKind::InvalidRegex,
                format!("AQL v1 不支持正则标志 '{flags}'"),
                Some("AQL v1 仅支持可选的 i 标志".to_owned()),
            ));
        }
        pattern.push(character);
    }

    Err(AqlError::at(
        lexer.source,
        start,
        lexer.offset,
        AqlErrorKind::InvalidToken,
        "正则字面量缺少结束 '/'",
        None,
    ))
}

/// 解析 `nth` 索引并拒绝超出当前平台 usize 的值。
fn lex_integer(lexer: &mut Lexer<'_>, start: usize) -> Result<TokenKind, AqlError> {
    while lexer.peek().is_some_and(|value| value.is_ascii_digit()) {
        lexer.bump();
    }
    let literal = &lexer.source[start..lexer.offset];
    literal
        .parse::<usize>()
        .map(TokenKind::Integer)
        .map_err(|_| {
            AqlError::at(
                lexer.source,
                start,
                lexer.offset,
                AqlErrorKind::InvalidArgument,
                "nth 索引超出可表示范围",
                None,
            )
        })
}

/// 解析 ASCII 标识符和属性 namespace。
fn lex_identifier(lexer: &mut Lexer<'_>) -> TokenKind {
    let start = lexer.offset;
    while lexer.peek().is_some_and(is_identifier_continue) {
        lexer.bump();
    }
    let identifier = &lexer.source[start..lexer.offset];
    match identifier {
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        _ => TokenKind::Identifier(identifier.to_owned()),
    }
}

/// AQL 标识符只允许稳定的 ASCII 语法字符。
const fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

/// namespace 属性额外允许 `.`，实际名称由 parser 白名单校验。
const fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '.'
}
