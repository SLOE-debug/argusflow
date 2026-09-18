//! 图片到阅读顺序文本块的 OCR 处理链。
mod cache;
mod detection;
mod incremental;
mod pipeline;
mod preprocessing;
mod recognition;
mod regions;
mod result;
pub(crate) use cache::RecognitionCache;
pub(crate) use pipeline::recognize;
pub use result::{OcrResult, TextBlock};
