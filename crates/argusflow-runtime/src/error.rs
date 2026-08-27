use crate::CommandError;
use crate::ValidationReport;
use argusflow_core::{
    ApplicationError, AutomationError, BrowserError, ResourceRef, WorkflowCapabilityId,
};
use thiserror::Error;

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
    /// 执行事件无法交付给调用方提供的接收器。
    #[error("workflow event could not be delivered: {0}")]
    EventSink(String),
    /// 校验后本应成立的运行时结构约束意外失效。
    #[error("validated workflow invariant failed: {0}")]
    ExecutionInvariant(String),
    /// 注册节点在自身领域执行边界返回的安全失败说明。
    #[error("registered node execution failed: {message}")]
    NodeExecution {
        /// 不包含敏感 payload、但可以交付给事件消费者的失败原因。
        message: String,
    },
    /// ValueExpr 引用的输入、变量或节点输出尚不可用。
    #[error("runtime value is unavailable: {description}")]
    ValueUnavailable {
        /// 无法解析的数据来源。
        description: String,
    },
    /// 结构化引用携带了不合法的 RFC 6901 JSON Pointer。
    #[error("invalid runtime JSON Pointer: {pointer}")]
    InvalidValuePointer {
        /// 持久化定义中的原始指针。
        pointer: String,
    },
    /// JSON Pointer 没有在所选数据源中命中值。
    #[error("runtime JSON Pointer did not match a value: {pointer}")]
    ValuePointerNotFound {
        /// 已通过格式校验但没有命中的指针。
        pointer: String,
    },
    /// 节点参数要求的类型与表达式实际结果不一致。
    #[error("runtime value type mismatch: expected {expected}, got {actual}")]
    ValueTypeMismatch {
        /// 节点参数所需的稳定类型名称。
        expected: &'static str,
        /// 不包含敏感业务内容的实际类型摘要。
        actual: &'static str,
    },
    /// 预编译表达式在受限作用域中求值失败。
    #[error("expression evaluation failed: {message}")]
    ExpressionEvaluation {
        /// Rhai 提供的安全错误摘要。
        message: String,
    },
    /// 表达式产生了不能回到 JSON 数据面的 Rhai 值。
    #[error("expression result is not a JSON value: {message}")]
    ExpressionResultNotJson {
        /// serde bridge 提供的类型错误摘要。
        message: String,
    },
    /// Set Variables 中的一个字段失败，整个赋值事务不会提交。
    #[error("variable assignment failed at node '{node_id}', variable '{variable}': {message}")]
    VariableAssignmentFailed {
        /// 发生失败的节点 ID。
        node_id: String,
        /// 未提交的变量名称。
        variable: String,
        /// 底层值解析错误摘要。
        message: String,
    },
    /// 节点自定义输出映射失败，原生与自定义输出都不会发布。
    #[error("output mapping failed at node '{node_id}', output '{output_name}': {message}")]
    OutputMappingFailed {
        /// 发生失败的节点 ID。
        node_id: String,
        /// 未发布的自定义输出名称。
        output_name: String,
        /// 底层值解析错误摘要。
        message: String,
    },
    /// 节点尝试使用工作流没有声明的系统能力。
    #[error("workflow capability was denied: {capability}")]
    CapabilityDenied {
        /// 被拒绝的稳定能力名称。
        capability: WorkflowCapabilityId,
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
    /// Chromium 浏览器会话获取或清理失败。
    #[error(transparent)]
    Browser(#[from] BrowserError),
    /// Command 节点准备或执行失败。
    #[error(transparent)]
    Command(#[from] CommandError),
    /// 自动化动作后端返回的结构化错误。
    #[error(transparent)]
    Automation(#[from] AutomationError),
}
