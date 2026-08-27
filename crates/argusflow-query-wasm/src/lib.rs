//! `argusflow-query` 到 WebView 的窄 WASM 语言服务边界。

use argusflow_query::{
    EditorPosition, code_actions as query_code_actions, completions as query_completions,
    hover as query_hover, inspect_document,
};
use wasm_bindgen::prelude::*;

/// 返回 lossless CST、诊断、semantic tokens、formatter 与 canonical identity。
#[wasm_bindgen]
pub fn inspect(source: &str) -> Result<JsValue, JsValue> {
    serialize(&inspect_document(source))
}

/// 返回指定 UTF-16 光标位置的 Rust grammar 补全候选。
#[wasm_bindgen]
pub fn completions(source: &str, line: u32, utf16_column: u32) -> Result<JsValue, JsValue> {
    serialize(&query_completions(
        source,
        EditorPosition { line, utf16_column },
    ))
}

/// 返回指定 UTF-16 光标位置的 Hover 数据。
#[wasm_bindgen]
pub fn hover(source: &str, line: u32, utf16_column: u32) -> Result<JsValue, JsValue> {
    serialize(&query_hover(source, EditorPosition { line, utf16_column }))
}

/// 返回当前文档可安全应用的 Rust grammar Code Action。
#[wasm_bindgen]
pub fn code_actions(source: &str) -> Result<JsValue, JsValue> {
    serialize(&query_code_actions(source))
}

/// 使用 `serde-wasm-bindgen` 保持 Rust 与 TypeScript 的结构化协议。
fn serialize<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| JsValue::from_str(&error.to_string()))
}
