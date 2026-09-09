//! 共享事件驱动桌面采样，像素处理由注入的平台后端提供。
mod config;
mod observation;
mod regions;
mod sampling;
mod service;

pub use config::{CaptureConfig, ObservationOptions};
pub use observation::{Anchor, Observation, ObservationStatus, ProcessSummary};
pub use regions::{normalize_regions, reading_regions, subtract_regions};
pub use service::{CaptureService, ChangeBatch, ChangeKind, ChangeRecord, ChangeSubscription};
