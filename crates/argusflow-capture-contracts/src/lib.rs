//! 与平台、OCR 和异步执行器无关的共享采样契约。
mod error;
mod frames;
mod geometry;
mod image;
mod resource;
mod sampling;
mod source;

pub use error::{CaptureError, CaptureResult};
pub use frames::{DesktopFrame, DesktopFrameSource, FrameConfig, FrameHistory};
pub use geometry::{PixelRect, Rotation, ScreenRect};
pub use image::{PixelFormat, PixelImage};
pub use resource::{ByteBudget, Reservation};
pub use sampling::{ContentToken, RegionSample, RegionSource, SampleContent, SampleRequest};
pub use source::{
    CaptureFuture, ClockDomain, ClockTime, Snapshot, SourceId, SourceInfo, SourceState,
    SourceValidity, Timing, Version,
};
