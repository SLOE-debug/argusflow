//! PP-OCRv6 原生图片识别，运行时不依赖 Python。

mod engine;
mod error;
mod image;
mod model;
mod ocr;
mod sampling;

pub use argusflow_core::OperationOptions;
pub use engine::{Device, ModelTier, OcrConfig, OcrEngine, OcrState};
pub use error::OcrError;
pub use image::{ImageInput, PixelFormat};
pub use ocr::{OcrResult, TextBlock};
pub use sampling::{SampledOcr, SampledOcrError, SampledOcrResult};
