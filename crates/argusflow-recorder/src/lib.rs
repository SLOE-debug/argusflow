//! Event-sourced user demonstration recorder：事件时间线与多模态证据。

#[cfg(test)]
mod click_contrast;
mod clipboard;
mod error;
mod event_clock;
mod evidence_reader;
#[cfg(test)]
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
mod privacy;
mod privacy_edit;
mod privacy_image;
mod redaction;
mod resolution;
mod sampling_wait;
mod screen_archive;
mod screen_archive_reader;
#[cfg(test)]
mod screen_archive_tests;
mod screen_archive_writer;
mod screen_association;
mod screen_privacy;
mod screen_recording;
mod screen_refinement;
mod screen_finalization;
mod screenshot_pipeline;
mod screenshots;
mod service;
mod status;
mod storage;
mod system_events;
mod trace;
mod worker;

pub use error::{CaptureStartupStage, RecorderError};
pub use history::RecordingSummary;
pub use input::{ClipboardContent, WindowChange};
pub use input::{InputPhase, KeyboardDecodeFailure, MouseButton, RawInput, RecordedText};
pub(crate) use input::{PhysicalEvent, PhysicalInput};
pub use pointer_motion::{MotionPoint, PointerMotion};
pub use privacy::RecordingPrivacy;
pub use privacy_edit::{PrivacyEdit, PrivacyRect};
pub use resolution::EvidenceCollector;
pub use screen_archive::{
    EventScreenEvidence, ScreenCompleteness, ScreenFrame, ScreenFrameId, ScreenPatch,
    ScreenTimeline, ScreenRefinement,
};
pub use screenshots::{ScreenshotCrop, ScreenshotEvidence, ScreenshotKind};
pub use service::{CompletedRecording, RecorderService};
pub use status::{RecorderPhase, RecorderStatus};
pub use storage::RecordingFiles;
pub use trace::*;

#[cfg(test)]
mod tests;
