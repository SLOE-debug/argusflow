//! OCR worker 生命周期、健康状态和控制面协议。

mod client;
mod health;
#[cfg(target_os = "windows")]
mod named_pipe;
mod protocol;

pub use client::{StaticOcrEngine, UnavailableOcrEngine, VisionWorkerClient};
pub use health::{RestartDecision, WorkerRestartPolicy, WorkerSupervisor};
#[cfg(target_os = "windows")]
pub use named_pipe::NamedPipeOcrEngine;
pub use protocol::{
    PixelTransport, VISION_PROTOCOL_VERSION, WorkerCommand, WorkerError, WorkerHealth,
    WorkerLifecycle, WorkerModelInfo, WorkerOcrRequest, WorkerProtocolEnvelope, WorkerResponse,
};
