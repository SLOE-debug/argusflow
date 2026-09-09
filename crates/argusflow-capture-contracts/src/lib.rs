//! 与平台、OCR 和异步执行器无关的共享采样契约。
mod error;
mod geometry;
mod image;
mod resource;
mod sampling;
mod source;

pub use error::{CaptureError, CaptureResult, Gap, GapReason};
pub use geometry::{PixelRect, Rotation, ScreenRect};
pub use image::{PixelFormat, PixelImage};
pub use resource::{ByteBudget, Reservation};
pub use sampling::{ContentToken, RegionSample, RegionSource, SampleContent, SampleRequest};
pub use source::{
    BackendConfig, BackendEvent, CaptureFuture, CaptureStats, ClockDomain, ClockTime,
    DesktopBackend, PixelChanges, Snapshot, SnapshotPixels, SourceId, SourceInfo, SourceState,
    Timing, Version,
};
