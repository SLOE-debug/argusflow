//! 中文编辑体验的 WASM 边界；词法转换与原生后端共享。
mod bridge;
mod completion;
mod hover;
mod service;
pub use argusflow_aql::{Translation, localize, translate};
pub use completion::{LocalizedCompletion, suggest};
pub use hover::{LocalizedHover, hover};
pub use service::{LocalizedDiagnostic, LocalizedDocument, inspect};
