//! OCR 文字快照的 AQL 适配；不负责采集或动作。
mod query;
pub use query::{OcrMatch, ocr_query_capabilities};
