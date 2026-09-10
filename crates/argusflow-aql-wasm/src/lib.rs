//! 中文编辑体验的 WASM 边界；不会参与原生后端编译与执行。
mod bridge;
mod completion;
mod hover;
mod localization;
mod service;
pub use completion::{LocalizedCompletion, suggest};
pub use hover::{LocalizedHover, hover};
pub use localization::{Translation, localize, translate};
pub use service::{LocalizedDiagnostic, LocalizedDocument, inspect};
