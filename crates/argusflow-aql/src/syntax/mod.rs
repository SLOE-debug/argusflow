//! 同一词法结果供解析与前端中文转换使用。
mod lexer;
mod parser;
mod predicate;
pub use lexer::{Lexed, Token, TokenKind, tokenize};
pub use parser::compile;
