//! 屏幕区域 OCR 的共享取图、在途合并和有界版本缓存。
mod cache;
mod model;
mod service;
pub use model::{SampledOcrError, SampledOcrResult};
pub use service::SampledOcr;
mod query;
