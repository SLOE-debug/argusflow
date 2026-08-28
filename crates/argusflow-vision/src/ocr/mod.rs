//! OCR 结果、模型 profile 与 Rust/Python worker 的稳定边界。

mod result;

pub use result::{
    OcrEngine, OcrItem, OcrModel, OcrOptions, OcrProfile, OcrRequest, OcrRequestId, OcrResponse,
    OcrSource, PolygonPoint, normalize_text,
};
