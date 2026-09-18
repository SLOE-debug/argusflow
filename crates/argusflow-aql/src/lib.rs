//! 平台无关的英文 AQL 编译、语言服务和有界树查询。
mod checking;
mod diagnostic;
mod evaluation;
mod language;
mod localization;
mod model;
mod syntax;

pub use diagnostic::{AqlError, DiagnosticCode, EditorPosition, EditorRange, Span};
pub use evaluation::preview;
pub use evaluation::{
    Capabilities, Node, NodeKind, QueryProgress, QueryTree, evaluate, evaluate_step,
};
pub use evaluation::{Geometry, Rect, SpatialCandidate, SpatialPreview};
pub use language::{
    Analysis, Completion, Hover, Symbol, SymbolKind, analyze, completions, format_query, hover,
    symbols,
};
pub use localization::{
    Translation, compile_target, is_target_symbol, localize, translate, translate_target,
};
pub use model::{
    Attribute, Bindings, BoundQuery, Boundary, CompiledQuery, Condition, Expr, MatchOperator,
    Operand, Relation, Role, Value, ValueType,
};
pub use model::{Length, LengthUnit, SpatialOptions, SpatialOrder, SpatialQuery};
pub use syntax::{Lexed, Token, TokenKind, compile, tokenize};
