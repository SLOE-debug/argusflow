//! AQL 的 lossless syntax、HIR lowering、语义工具与后端计划共享契约。
//!
//! 本 crate 依赖 `argusflow-core` 的平台无关 AST，不依赖任何 UIA、CDP 或视觉实现。

mod alternative;
mod analyze;
mod capability;
mod diagnostic;
mod error;
mod formatter;
mod language;
mod lexer;
mod normalize;
mod observation_eval;
mod observation_formatter;
mod observation_resolve;
mod parser;
mod protocol;
mod resolve;
mod syntax;

pub use alternative::{
    AlternativeBudgetExceeded, AlternativeExpansionBudget, MAX_COMPILED_ALTERNATIVES,
};
pub use analyze::{QueryAnalysis, analyze_query};
pub use capability::{
    BackendQueryCapability, BranchPath, QueryBackend, QueryCost, QueryPortability, SupportLevel,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticParams, DiagnosticSeverity};
pub use error::{AqlError, AqlErrorKind, SourceSpan};
pub use formatter::{canonicalize_query, format_query, format_source};
pub use language::{
    LanguageDocument, ParsedDocument, code_actions, completions, hover, inspect_document,
    parse_document,
};
pub use normalize::normalize_query;
pub use observation_eval::{evaluate_observation, observation_selectors};
pub use observation_formatter::{canonicalize_observation, format_observation};
pub use observation_resolve::{
    ObservationParameterError, observation_parameter_types, resolve_observation_parameters,
};
pub use parser::{
    parse_observation_query, parse_query, parse_stored_observation, parse_stored_query,
};
pub use protocol::{
    CompletionItem, CompletionItemKind, EditorPosition, EditorRange, Hover, SyntaxToken,
    SyntaxTokenKind, TextEdit, byte_range_to_editor_range,
};
pub use resolve::{QueryParameterResolutionError, query_parameter_names, resolve_query_parameters};
pub use syntax::{CstElement, CstNode, CstNodeKind, RawToken, RawTokenKind, SyntaxTree};
pub(crate) use syntax::{build_recovery_tree, lex_lossless};
