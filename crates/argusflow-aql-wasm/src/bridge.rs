//! wasm-bindgen 仅负责序列化和导出编排。
use crate::{hover, inspect, localize, suggest};
use argusflow_aql::EditorPosition;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = inspect)]
/// 分析中文草稿。
pub fn inspect_js(source: &str) -> Result<JsValue, JsValue> {
    serialize(&inspect(source))
}
#[wasm_bindgen(js_name = completions)]
/// 获取指定位置的中文补全。
pub fn completions_js(source: &str, line: u32, column: u32) -> Result<JsValue, JsValue> {
    serialize(&suggest(source, EditorPosition { line, column }))
}
#[wasm_bindgen(js_name = hover)]
/// 获取中文悬浮说明。
pub fn hover_js(source: &str, line: u32, column: u32) -> Result<JsValue, JsValue> {
    serialize(&hover(source, EditorPosition { line, column }))
}
#[wasm_bindgen(js_name = importEnglish)]
/// 将导入的英文 AQL 转换成可编辑中文文本。
pub fn import_english(source: &str) -> String {
    localize(source).source().into()
}
fn serialize(value: &impl serde::Serialize) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|error| JsValue::from_str(&error.to_string()))
}
