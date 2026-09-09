//! 与平台、录制和 OCR 无关的精确像素变化基础设施。

mod desktop_hub;
mod desktop_pump;
mod diff;
pub use desktop_hub::{CaptureScheduler, DesktopCaptureHub, DesktopSubscription};
mod regions;
pub use regions::merge_regions;
mod broker;
mod kernel;
mod shared_window;
mod snapshot;
mod source;
mod stream;
pub use shared_window::{SharedWindowSource, exact_regions};

#[cfg(test)]
mod desktop_hub_tests;
#[cfg(test)]
mod shared_window_tests;
#[cfg(test)]
mod tests;

pub use argusflow_core::CaptureError;
pub use argusflow_core::capture::frame::{
    FrameId, PhysicalRect, PixelFormat, QpcTimestamp, TopologyGeneration,
};
pub use argusflow_core::capture::image::{CapturedFrame, PixelImage};
pub use broker::CaptureBroker;
pub use diff::{ChangeSet, PixelRect, PixelView, compare};
pub use snapshot::{FrameSnapshot, PixelBlock};
pub use source::{
    CaptureHealth, CaptureLifecycle, CapturePolicy, FrameSubscription, MemoryFrameSource,
    WindowFrameSource,
};
pub use stream::{CaptureCursor, CaptureStream, FrameDelivery, FrameUpdate};
