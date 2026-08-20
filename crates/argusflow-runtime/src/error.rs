use argusflow_core::AutomationError;
use thiserror::Error;
use uuid::Uuid;

use crate::ValidationReport;

/// 工作流运行时在校验、调度或事件交付阶段返回的错误。
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// 工作流未通过结构或参数校验。
    #[error("workflow validation failed")]
    ValidationFailed {
        /// 包含所有校验失败项，供调用方展示或记录。
        report: ValidationReport,
    },
    /// 当前引擎已有尚未结束的运行，拒绝并发启动。
    #[error("workflow run {run_id} is already active")]
    RunInProgress {
        /// 当前仍处于活动状态的运行 ID。
        run_id: Uuid,
    },
    /// 执行事件无法交付给调用方提供的接收器。
    #[error("workflow event could not be delivered: {0}")]
    EventSink(String),
    /// 校验后本应成立的运行时结构约束意外失效。
    #[error("validated workflow invariant failed: {0}")]
    ExecutionInvariant(String),
    /// 自动化动作后端返回的结构化错误。
    #[error(transparent)]
    Automation(#[from] AutomationError),
}
