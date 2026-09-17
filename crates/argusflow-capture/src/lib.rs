//! 完整帧来源的区域稳定采样；不持有 GPU 历史、变化日志或 OCR 引擎。
mod pixels;
mod sampling;
mod validity;
pub use sampling::FrameSampler;
