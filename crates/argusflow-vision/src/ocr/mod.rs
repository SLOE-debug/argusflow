//! 图片到阅读顺序文本块的 OCR 处理链。
mod detection;
mod pipeline;
mod preprocessing;
mod recognition;
mod result;
pub(crate) use pipeline::recognize;
pub use result::{OcrResult, TextBlock};
