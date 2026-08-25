//! AQL 的 lossless syntax、HIR lowering、语义工具与后端计划共享契约。
//!
//! 本 crate 依赖 `argusflow-core` 的平台无关 AST，不依赖任何 UIA、CDP 或视觉实现。

mod analyze;
mod capability;
mod diagnostic;
mod error;
mod formatter;
mod language;
mod lexer;
mod normalize;
mod parser;
mod protocol;
mod syntax;

pub use analyze::{QueryAnalysis, analyze_query};
pub use capability::{
    BackendQueryCapability, BranchPath, QueryBackend, QueryCost, QueryPortability, SupportLevel,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticParams, DiagnosticSeverity};
pub use error::{AqlError, AqlErrorKind, SourceSpan};
pub use formatter::{canonicalize_query, format_query, format_source};
pub use language::{
    LanguageDocument, ParsedDocument, bracket_pair, code_actions, completions, hover,
    inspect_document, parse_document,
};
pub use normalize::normalize_query;
pub use parser::{parse_query, parse_stored_query};
pub use protocol::{
    CompletionItem, CompletionItemKind, EditorPosition, EditorRange, Hover, SyntaxToken,
    SyntaxTokenKind, TextEdit, byte_range_to_editor_range,
};
pub use syntax::{CstElement, CstNode, CstNodeKind, RawToken, RawTokenKind, SyntaxTree};
pub(crate) use syntax::{build_recovery_tree, lex_lossless};
