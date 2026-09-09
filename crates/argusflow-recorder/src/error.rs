//! 录制控制面错误；不返回输入内容或 provider 原始数据。

use thiserror::Error;

/// 屏幕录制启动失败的职责边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStartupStage {
    /// 共享原生主机建立完整桌面基准。
    DesktopBaseline,
    /// 读取采集诊断计数。
    CaptureCounters,
    /// 注册有序变化订阅。
    OrderedSubscription,
    /// 基准图像编码并提交归档。
    ArchiveBaseline,
}

/// 开始、停止与本地持久化的明确错误。
#[derive(Debug, Error)]
pub enum RecorderError {
    /// GPU 或离线归档精确化失败；原始候选归档仍保留。
    #[error("screen refinement failed: {0}")]
    Refinement(String),
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
    /// 屏幕采集启动阶段及底层失败原因，避免被工作线程错误掩盖。
    #[error("screen capture startup failed ({stage:?}): {reason:?}")]
    CaptureStartup {
        /// 失败的初始化阶段。
        stage: CaptureStartupStage,
        /// 共享采集或归档返回的明确失败分类。
        reason: argusflow_core::CaptureFailure,
    },
    /// 本地文件写入失败。
    #[error("recording persistence failed: {0}")]
    Storage(#[from] std::io::Error),
    /// Trace DTO 序列化失败。
    #[error("recording serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}
