//! 工作流校验、启动及执行事件转发命令。

use std::sync::Arc;

use argusflow_core::{ExecutionEvent, RunStarted, WorkflowDefinition};
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
) -> Result<RunStarted, CommandError> {
    // 将运行时事件桥接到当前 Tauri 应用，供前端实时订阅执行进度。
    let sink = Arc::new(TauriEventSink { app });
    state
        .engine
        .start(workflow, sink)
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
    pub code: &'static str,
    /// 适合展示或记录的错误说明。
    pub message: String,
    /// 工作流校验失败时的详细问题；其他错误类型为空。
    pub issues: Vec<ValidationIssue>,
}

impl From<RuntimeError> for CommandError {
    fn from(error: RuntimeError) -> Self {
        match error {
            RuntimeError::ValidationFailed { report } => Self {
                code: "validation_failed",
                message: "工作流校验失败".to_owned(),
                issues: report.issues,
            },
            RuntimeError::RunInProgress { run_id } => Self {
                code: "run_in_progress",
                message: format!("工作流运行 {run_id} 尚未结束"),
                issues: Vec::new(),
            },
            RuntimeError::EventSink(message) => Self {
                code: "event_delivery_failed",
                message,
                issues: Vec::new(),
            },
            RuntimeError::ExecutionInvariant(message) => Self {
                code: "execution_invariant_failed",
                message,
                issues: Vec::new(),
            },
            RuntimeError::Automation(error) => Self {
                code: "automation_failed",
                message: error.to_string(),
                issues: Vec::new(),
            },
        }
    }
}
