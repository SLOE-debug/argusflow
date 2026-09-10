//! 可编译为 WASM 的同源语言服务。
mod documentation;
mod formatter;
mod hover;
mod service;
mod symbols;
pub use formatter::format_query;
pub use hover::{Hover, hover};
pub use service::{Analysis, Completion, analyze, completions};
pub use symbols::{Symbol, SymbolKind, symbols};
