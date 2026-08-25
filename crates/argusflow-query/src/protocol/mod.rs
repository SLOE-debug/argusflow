//! AQL 语言服务跨 Rust、WASM 与 WebView 使用的稳定协议。

mod range;
mod token;

pub use range::{EditorPosition, EditorRange, byte_range_to_editor_range};
pub use token::{
    CompletionItem, CompletionItemKind, Hover, SyntaxToken, SyntaxTokenKind, TextEdit,
};
