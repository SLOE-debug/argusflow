//! 保留 trivia、错误 token 与不完整结构的 AQL 具体语法层。

mod cst;
mod lexer;
mod parser;

pub use cst::{CstElement, CstNode, CstNodeKind, RawToken, RawTokenKind, SyntaxTree};
pub(crate) use lexer::lex_lossless;
pub(crate) use parser::build_recovery_tree;
