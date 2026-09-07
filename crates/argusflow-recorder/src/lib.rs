//! Event-sourced user demonstration recorder：事件时间线与多模态证据。

mod clipboard;
mod error;
mod event_clock;
mod evidence_reader;
mod history;
mod hooks;
mod ingestion;
mod input;
mod keyboard;
mod motion_compaction;
mod motion_simplification;
mod pointer_motion;
mod redaction;
mod resolution;
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
pub use resolution::EvidenceCollector;
pub use screenshots::{ScreenshotCrop, ScreenshotEvidence, ScreenshotKind};
pub use service::{CompletedRecording, RecorderService};
pub use status::{RecorderPhase, RecorderStatus};
pub use storage::RecordingFiles;
pub use trace::*;

#[cfg(test)]
mod tests;
