//! 桌面 AI 装配：本机配置、录制包授权范围和 IPC 生命周期。
mod commands;
mod jobs;
mod result;
pub(crate) use commands::*;
pub(crate) use jobs::AiJobs;
