//! 基于原始序号的有界操作归一化。
mod service;
mod writer;
pub use service::Normalizer;
pub use writer::RecordingWriter;
mod association;
