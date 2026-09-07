//! 录制控制面错误；不返回输入内容或 provider 原始数据。

use thiserror::Error;

/// 开始、停止与本地持久化的明确错误。
#[derive(Debug, Error)]
pub enum RecorderError {
    /// Manifest 身份、版本或事件计数不一致。
    #[error("recording manifest does not match its trace")]
    InvalidRecording,
    /// 同一控制器只能拥有一个全局 Hook session。
    #[error("a recording is already active")]
    AlreadyRecording,
    /// 无正在录制的会话。
    #[error("no recording is active")]
    NotRecording,
    /// Hook 安装、消息线程或卸载失败。
    #[error("Windows recorder hook unavailable")]
    HookUnavailable,
    /// 异步 worker 异常退出。
    #[error("recorder worker unavailable")]
    WorkerUnavailable,
    /// 本地文件写入失败。
    #[error("recording persistence failed: {0}")]
    Storage(#[from] std::io::Error),
    /// Trace DTO 序列化失败。
    #[error("recording serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}
