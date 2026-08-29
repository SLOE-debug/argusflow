//! OCR 结果、模型 profile 与 Rust/Python worker 的稳定边界。

mod result;

pub use result::{
    OcrDiagnosticImageEncoding, OcrEngine, OcrImagePreprocessing, OcrItem, OcrModel,
    OcrModelInputArtifact, OcrOptions, OcrPreprocessingSummary, OcrProfile, OcrRequest,
    OcrRequestId, OcrResponse, OcrTimingSummary, PolygonPoint, normalize_text,
};
