//! Event-sourced user demonstration recorder：事件时间线与多模态证据。

mod capture_settle;
mod click_contrast;
mod clipboard;
mod error;
mod event_clock;
mod evidence_reader;
mod evidence_region;
#[cfg(test)]
mod evidence_region_tests;
mod history;
mod hooks;
mod ingestion;
mod input;
mod keyboard;
mod motion_compaction;
mod motion_simplification;
mod pointer_motion;
mod post_capture;
mod privacy;
mod privacy_edit;
mod privacy_image;
mod redaction;
mod resolution;
#[cfg(test)]
mod screen_diff;
mod screen_diff_kernel;
mod screen_observer;
mod screenshot_pipeline;
mod screenshots;
mod service;
mod status;
mod storage;
mod system_events;
mod trace;
mod worker;

pub use error::RecorderError;
pub use history::RecordingSummary;
pub use input::{ClipboardContent, WindowChange};
pub use input::{InputPhase, KeyboardDecodeFailure, MouseButton, RawInput, RecordedText};
pub(crate) use input::{PhysicalEvent, PhysicalInput};
pub use pointer_motion::{MotionPoint, PointerMotion};
pub use privacy::RecordingPrivacy;
pub use privacy_edit::{PrivacyEdit, PrivacyRect};
pub use resolution::EvidenceCollector;
pub use screenshots::{ScreenshotCrop, ScreenshotEvidence, ScreenshotKind};
pub use service::{CompletedRecording, RecorderService};
pub use status::{RecorderPhase, RecorderStatus};
pub use storage::RecordingFiles;
pub use trace::*;

#[cfg(test)]
mod capture_settle_tests;
#[cfg(test)]
mod post_capture_tests;
#[cfg(test)]
mod tests;
