//! 平台无关的英文 AQL 编译、语言服务和有界树查询。
mod checking;
mod diagnostic;
mod evaluation;
mod language;
mod model;
mod syntax;

pub use diagnostic::{AqlError, DiagnosticCode, EditorPosition, EditorRange, Span};
pub use evaluation::{
    Capabilities, Node, NodeKind, QueryProgress, QueryTree, evaluate, evaluate_step,
};
pub use language::{
    Analysis, Completion, Hover, Symbol, SymbolKind, analyze, completions, format_query, hover,
    symbols,
};
pub use model::{
    Attribute, Bindings, BoundQuery, Boundary, CompiledQuery, Condition, Expr, MatchOperator,
    Operand, Relation, Role, Value, ValueType,
};
pub use syntax::{Lexed, Token, TokenKind, compile, tokenize};
