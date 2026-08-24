//! AQL v1 的 lexer、parser、normalizer、formatter 与 capability analyzer。
//!
//! 本 crate 依赖 `argusflow-core` 的平台无关 AST，不依赖任何 UIA、CDP 或视觉实现。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod analyze;
mod capability;
mod error;
mod formatter;
mod lexer;
mod normalize;
mod parser;

pub use analyze::{QueryAnalysis, analyze_query};
pub use capability::{
    BackendQueryCapability, QueryBackend, QueryCost, QueryPortability, QueryWarning,
    QueryWarningKind, SupportLevel,
};
pub use error::{AqlError, AqlErrorKind, SourceSpan};
pub use formatter::{canonicalize_query, format_query};
pub use normalize::normalize_query;
pub use parser::{parse_query, parse_stored_query};
