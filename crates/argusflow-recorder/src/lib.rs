//! Semantic Recorder：Physical → Semantic，与执行路由职责独立。

mod error;
mod event_clock;
mod history;
mod hooks;
mod ingestion;
mod input;
mod keyboard;
mod normalization;
mod redaction;
mod resolution;
mod selectors;
mod service;
mod status;
mod storage;
mod trace;
mod worker;

pub use error::RecorderError;
pub use history::RecordingSummary;
pub use input::{InputPhase, KeyboardDecodeFailure, MouseButton, RawInput, RecordedText};
pub(crate) use input::{PhysicalEvent, PhysicalInput};
pub use normalization::TraceNormalizer;
pub use resolution::TargetResolver;
pub use selectors::{CandidateBasis, RecordedSelector, SelectorCandidate, synthesize_selectors};
pub use service::{CompletedRecording, RecorderService};
pub use status::{RecorderPhase, RecorderStatus};
pub use storage::RecordingFiles;
pub use trace::*;

#[cfg(test)]
mod tests;
