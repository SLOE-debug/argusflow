//! Windows UI Automation 与显式真实输入基础能力。

#![cfg(windows)]

mod application;
mod capture;
mod error;
mod input;
mod platform;
mod uia;
mod window;

pub use application::{Application, ApplicationOptions};
pub use argusflow_core::OperationOptions;
pub use capture::DxgiFrameSource;
pub use capture::video::{
    DecodedVideoFrame, VideoDecoder, VideoError, VideoOptions, VideoReport, VideoThumbnail,
    decode_video_frame, record_desktop_video, record_desktop_video_until,
};
pub use error::WindowsError;
pub use input::{InputAction, InputSequence, InputService, InputState};
pub use uia::{UiaObservation, UiaObservedNode};
pub mod listening;
pub use uia::{
    ControlType, ElementHandle, ElementSnapshot, Predicate, Query, ScrollAmount, SearchScope,
    SelectionAction, UiaAction, UiaConfig, UiaMatch, UiaRuntime, UiaState,
};
pub use window::{WindowIdentity, WindowInfo, WindowLocator};
