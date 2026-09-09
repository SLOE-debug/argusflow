//! OCR 配置、串行工作线程及模型实例所有权。
mod config;
mod ownership;
mod service;
pub use config::{Device, ModelTier, OcrConfig};
pub use service::{OcrEngine, OcrState};
