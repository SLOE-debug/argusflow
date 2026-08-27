//! 工作流校验、启动及执行事件转发命令。

use std::sync::Arc;

use argusflow_core::{ExecutionEvent, RunInputs, RunStarted, WorkflowDefinition};
use argusflow_runtime::{
    ExecutionEventSink, RuntimeError, ValidationIssue, ValidationReport,
    validate_workflow as validate,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::runtime::AppState;

/// 前端订阅工作流执行事件时使用的稳定事件名称。
pub const WORKFLOW_EVENT_NAME: &str = "argusflow://workflow-event";

#[tauri::command]
/// 校验工作流定义并返回所有发现的问题，不启动执行。
pub fn validate_workflow(workflow: WorkflowDefinition) -> ValidationReport {
    validate(&workflow)
}

#[tauri::command]
/// 启动工作流执行，并通过 Tauri 事件流回传执行进度。
///
/// 当已有运行中的工作流或校验失败时，错误会转换为前端可序列化的代码与问题列表。
pub async fn run_workflow(
    app: AppHandle,
    state: State<'_, AppState>,
    workflow: WorkflowDefinition,
    inputs: RunInputs,
) -> Result<RunStarted, CommandError> {
    // 将运行时事件桥接到当前 Tauri 应用，供前端实时订阅执行进度。
    let sink = Arc::new(TauriEventSink { app });
    state
        .engine
        .start(workflow, inputs, sink)
        .await
        .map_err(CommandError::from)
}

struct TauriEventSink {
    app: AppHandle,
}

impl ExecutionEventSink for TauriEventSink {
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.app
            .emit(WORKFLOW_EVENT_NAME, event)
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Serialize)]
/// 面向 Tauri 调用方的统一命令错误载荷。
pub struct CommandError {
    /// 稳定的机器可读错误代码。
    pub code: CommandErrorCode,
    /// 适合展示或记录的错误说明。
    pub message: String,
    /// 工作流校验失败时的详细问题；其他错误类型为空。
    pub issues: Vec<ValidationIssue>,
}

/// Tauri 工作流命令返回的稳定错误类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorCode {
    /// 工作流定义未通过运行前校验。
    ValidationFailed,
    /// 执行事件无法投递给前端。
    EventDeliveryFailed,
    /// 校验后依赖的结构约束在执行期间失效。
    ExecutionInvariantFailed,
    /// 自动化后端执行动作失败。
    AutomationFailed,
    /// 应用资源获取或清理失败。
    ApplicationFailed,
    /// 浏览器资源获取、CDP 初始化或清理失败。
    BrowserFailed,
    /// 命令节点准备或执行失败。
    CommandFailed,
    /// 节点的数据或资源引用在执行期不可用。
    RuntimeDataFailed,
}

impl From<RuntimeError> for CommandError {
    fn from(error: RuntimeError) -> Self {
        match error {
            RuntimeError::ValidationFailed { report } => Self {
                code: CommandErrorCode::ValidationFailed,
                message: "工作流校验失败".to_owned(),
                issues: report.issues,
            },
            RuntimeError::InvalidRunInputs { message } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message,
                issues: Vec::new(),
            },
            RuntimeError::EventSink(message) => Self {
                code: CommandErrorCode::EventDeliveryFailed,
                message,
                issues: Vec::new(),
            },
            RuntimeError::ExecutionInvariant(message) => Self {
                code: CommandErrorCode::ExecutionInvariantFailed,
                message,
                issues: Vec::new(),
            },
            RuntimeError::NodeExecution { message } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message,
                issues: Vec::new(),
            },
            RuntimeError::ValueUnavailable { description } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: description,
                issues: Vec::new(),
            },
            RuntimeError::InvalidValuePointer { pointer } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("运行时 JSON Pointer 格式无效：{pointer}"),
                issues: Vec::new(),
            },
            RuntimeError::ValuePointerNotFound { pointer } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("运行时 JSON Pointer 未匹配到值：{pointer}"),
                issues: Vec::new(),
            },
            RuntimeError::ValueTypeMismatch { expected, actual } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("运行时值类型不匹配，需要 {expected}，实际为 {actual}"),
                issues: Vec::new(),
            },
            RuntimeError::ExpressionEvaluation { message } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("表达式求值失败：{message}"),
                issues: Vec::new(),
            },
            RuntimeError::ExpressionResultNotJson { message } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("表达式结果不是 JSON 值：{message}"),
                issues: Vec::new(),
            },
            RuntimeError::VariableAssignmentFailed {
                node_id,
                variable,
                message,
            } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("节点 {node_id} 的变量 {variable} 赋值失败：{message}"),
                issues: Vec::new(),
            },
            RuntimeError::OutputMappingFailed {
                node_id,
                output_name,
                message,
            } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("节点 {node_id} 的输出 {output_name} 映射失败：{message}"),
                issues: Vec::new(),
            },
            RuntimeError::CapabilityDenied { capability } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!("工作流能力未授权：{}", capability.as_str()),
                issues: Vec::new(),
            },
            RuntimeError::ResourceUnavailable { reference } => Self {
                code: CommandErrorCode::RuntimeDataFailed,
                message: format!(
                    "运行时资源不可用：{}.{}",
                    reference.producer_node_id, reference.output_name,
                ),
                issues: Vec::new(),
            },
            RuntimeError::Application(error) => Self {
                code: CommandErrorCode::ApplicationFailed,
                message: error.to_string(),
                issues: Vec::new(),
            },
            RuntimeError::Browser(error) => Self {
                code: CommandErrorCode::BrowserFailed,
                message: error.to_string(),
                issues: Vec::new(),
            },
            RuntimeError::Command(error) => Self {
                code: CommandErrorCode::CommandFailed,
                message: error.to_string(),
                issues: Vec::new(),
            },
            RuntimeError::Automation(error) => Self {
                code: CommandErrorCode::AutomationFailed,
                message: error.to_string(),
                issues: Vec::new(),
            },
        }
    }
}
