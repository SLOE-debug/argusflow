//! 来源身份、不可变版本以及原生后台能力。
mod backend;
mod model;
pub use backend::{
    BackendConfig, BackendEvent, CaptureFuture, CaptureStats, DesktopBackend, SnapshotPixels,
};
pub use model::{
    ClockDomain, ClockTime, PixelChanges, Snapshot, SourceId, SourceInfo, SourceState, Timing,
    Version,
};
