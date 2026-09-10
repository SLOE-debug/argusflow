//! 有界递归下降解析器，只接受英文 AQL 源码。
use super::tokenize;
use crate::{
    AqlError, Boundary, CompiledQuery, DiagnosticCode, Expr, Relation, Role, Span, Token,
    TokenKind, ValueType,
};
use std::{collections::BTreeMap, num::NonZeroUsize};

/// 编译英文 AQL；参数类型和正则在任何平台 I/O 之前检查。
pub fn compile(source: &str) -> Result<CompiledQuery, AqlError> {
    let lexed = tokenize(source);
    if let Some(error) = lexed.diagnostics.into_iter().next() {
        return Err(error);
    }
    let tokens = lexed
        .tokens
        .into_iter()
        .filter(|t| !matches!(t.kind, TokenKind::Whitespace | TokenKind::Comment))
        .collect();
    let mut parser = Parser {
        source,
        tokens,
        position: 0,
        depth: 0,
        nodes: 0,
        parameters: BTreeMap::new(),
    };
    let expression = parser.relation()?;
    if parser.position != parser.tokens.len() {
        return Err(parser.error(DiagnosticCode::Syntax, "查询结束后存在多余内容"));
    }
    crate::checking::validate(&expression)?;
    Ok(CompiledQuery::new(expression, parser.parameters))
}
pub(super) struct Parser<'a> {
    pub(super) source: &'a str,
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
    nodes: usize,
    pub(super) parameters: BTreeMap<String, ValueType>,
}
impl Parser<'_> {
    pub(super) fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }
    pub(super) fn word(&self) -> &str {
        self.current().map(|t| t.text(self.source)).unwrap_or("")
    }
    pub(super) fn advance(&mut self) {
        self.position += 1;
    }
    pub(super) fn take(&mut self, word: &str) -> bool {
        if self.word() == word {
            self.advance();
            true
        } else {
            false
        }
    }
    pub(super) fn expect(&mut self, word: &str) -> Result<(), AqlError> {
        if self.take(word) {
            Ok(())
        } else {
            Err(self.error(DiagnosticCode::Syntax, format!("此处需要 {word}")))
        }
    }
    pub(super) fn error(&self, code: DiagnosticCode, message: impl Into<String>) -> AqlError {
        AqlError::new(
            code,
            self.current()
                .map(|t| t.span)
                .unwrap_or(Span::new(self.source.len(), self.source.len())),
            message,
        )
    }
    pub(super) fn enter(&mut self) -> Result<(), AqlError> {
        self.depth += 1;
        self.nodes += 1;
        if self.depth > 64 || self.nodes > 256 {
            return Err(self.error(
                DiagnosticCode::Limit,
                "查询嵌套超过 64 层或表达式超过 256 项",
            ));
        }
        Ok(())
    }
    pub(super) fn leave(&mut self) {
        self.depth -= 1;
    }
    fn relation(&mut self) -> Result<Expr, AqlError> {
        self.enter()?;
        let mut left = self.primary()?;
        while matches!(self.word(), ">" | ">>") {
            self.nodes += 1;
            if self.nodes > 256 {
                return Err(self.error(DiagnosticCode::Limit, "查询关系过多"));
            }
            let relation = if self.take(">>") {
                Relation::Descendant
            } else {
                self.expect(">")?;
                Relation::Child
            };
            left = Expr::Relation {
                left: Box::new(left),
                right: Box::new(self.primary()?),
                relation,
            };
        }
        self.leave();
        Ok(left)
    }
    fn primary(&mut self) -> Result<Expr, AqlError> {
        self.enter()?;
        if self.take("(") {
            let expression = self.relation()?;
            self.expect(")")?;
            self.leave();
            return Ok(expression);
        }
        let name = self.word().to_owned();
        if !self
            .current()
            .is_some_and(|t| t.kind == TokenKind::Identifier)
        {
            return Err(self.error(DiagnosticCode::Syntax, "此处需要角色或查询函数"));
        }
        let token_span = self.current().map(|t| t.span).unwrap_or_default();
        self.advance();
        self.expect("(")?;
        let expression = match name.as_str() {
            "first" | "nth" => {
                let query = self.relation()?;
                let index = if name == "first" {
                    NonZeroUsize::MIN
                } else {
                    self.expect(",")?;
                    let index = self
                        .word()
                        .parse::<usize>()
                        .ok()
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| {
                            self.error(DiagnosticCode::Type, "nth 的索引必须是从 1 开始的整数")
                        })?;
                    self.advance();
                    index
                };
                Expr::Nth {
                    query: Box::new(query),
                    index,
                }
            }
            "css" => {
                if !self.current().is_some_and(|t| t.kind == TokenKind::String) {
                    return Err(self.error(DiagnosticCode::Type, "css 需要字符串字面量"));
                }
                let selector: String = serde_json::from_str(self.word())
                    .map_err(|_| self.error(DiagnosticCode::Lexical, "字符串转义无效"))?;
                if selector.is_empty() || selector.len() > 16_384 {
                    return Err(self.error(DiagnosticCode::Limit, "CSS 必须为 1 到 16384 字节"));
                }
                self.advance();
                Expr::Css(selector)
            }
            "frame" | "shadow" => Expr::Enter {
                host: Box::new(self.relation()?),
                boundary: if name == "frame" {
                    Boundary::Frame
                } else {
                    Boundary::Shadow
                },
            },
            _ => {
                let role = Role::parse(&name).ok_or_else(|| {
                    AqlError::new(
                        DiagnosticCode::UnknownSymbol,
                        token_span,
                        "未知角色或函数；后端只接受英文 AQL 关键字",
                    )
                })?;
                let condition = if self.word() == ")" {
                    None
                } else {
                    Some(self.condition()?)
                };
                Expr::Match { role, condition }
            }
        };
        self.expect(")")?;
        self.leave();
        Ok(expression)
    }
}
