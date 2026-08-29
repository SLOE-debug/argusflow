//! OCR worker 生命周期、健康状态和控制面协议。

mod client;
#[cfg(target_os = "windows")]
mod named_pipe;
mod protocol;
#[cfg(target_os = "windows")]
mod shared_memory;

pub use client::{StaticOcrEngine, UnavailableOcrEngine, VisionWorkerClient};
#[cfg(target_os = "windows")]
pub use named_pipe::NamedPipeOcrEngine;
pub use protocol::{
    MAX_PIXEL_BODY_BYTES, SharedMemoryPixels, VISION_PROTOCOL_VERSION, WorkerBinaryArtifact,
    WorkerBinaryArtifactKind, WorkerCommand, WorkerDiagnosticsRequest, WorkerError, WorkerHealth,
    WorkerLifecycle, WorkerModelInfo, WorkerOcrRequest, WorkerProtocolEnvelope, WorkerResponse,
};
