//! 单调时钟、来源身份与固定图像的有效性。
mod backend;
mod model;
pub use backend::{CaptureFuture, SourceValidity};
pub use model::{
    ClockDomain, ClockTime, Snapshot, SourceId, SourceInfo, SourceState, Timing, Version,
};
