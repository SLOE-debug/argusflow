use argusflow_core::{ApplicationError, AutomationError, ResourceRef, WorkflowCapability};
use thiserror::Error;
use uuid::Uuid;

use crate::CommandError;
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
    /// 调用方提供的本次运行输入与工作流声明不一致。
    #[error("invalid workflow run inputs: {message}")]
    InvalidRunInputs {
        /// 缺失、多余或类型错误的输入说明。
        message: String,
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
    /// ValueExpr 引用的输入、变量或节点输出尚不可用。
    #[error("runtime value is unavailable: {description}")]
    ValueUnavailable {
        /// 无法解析的数据来源。
        description: String,
    },
    /// 节点参数要求字符串，但表达式解析成了其它 JSON 类型。
    #[error("runtime value type mismatch: expected {expected}")]
    ValueTypeMismatch {
        /// 节点参数所需的稳定类型名称。
        expected: &'static str,
    },
    /// 节点尝试使用工作流没有声明的系统能力。
    #[error("workflow capability was denied: {capability}")]
    CapabilityDenied {
        /// 被拒绝的稳定能力名称。
        capability: WorkflowCapability,
    },
    /// ResourceRef 在当前运行中没有绑定真实资源。
    #[error("runtime resource is unavailable: {}.{}", reference.producer_node_id, reference.output_name)]
    ResourceUnavailable {
        /// 无法解析的逻辑资源引用。
        reference: ResourceRef,
    },
    /// 平台应用会话获取或清理失败。
    #[error(transparent)]
    Application(#[from] ApplicationError),
    /// Command 节点准备或执行失败。
    #[error(transparent)]
    Command(#[from] CommandError),
    /// 自动化动作后端返回的结构化错误。
    #[error(transparent)]
    Automation(#[from] AutomationError),
}
