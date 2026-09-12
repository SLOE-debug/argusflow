//! Tauri 只适配参数并编排独立服务。
use crate::{
    document::{DocumentSummary, LoadedDocument, WorkflowFile, Workspace},
    runtime::{RunManager, RunMessage, load_bundle},
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State, ipc::Channel};
use tokio::sync::Mutex;

/// 桌面状态仅持有服务，不实现领域规则。
#[derive(Default)]
pub struct DesktopState {
    /// 固定应用数据目录内的工作流存储。
    pub workspace: Mutex<Option<Workspace>>,
    /// 跨页面生命周期的运行服务。
    pub runs: Arc<RunManager>,
}
#[derive(Serialize)]
pub struct OpenedWorkspace {
    path: String,
    documents: Vec<DocumentSummary>,
}
#[tauri::command]
pub async fn initialize_workspace(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<OpenedWorkspace, String> {
    // 初始化锁同时保护检查与安装，重复调用不会替换活动运行使用的工作区。
    let mut slot = state.workspace.lock().await;
    if let Some(workspace) = slot.as_ref() {
        return Ok(OpenedWorkspace {
            path: workspace.path(),
            documents: workspace.list()?,
        });
    }
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?
        .join("workflows");
    let workspace = Workspace::initialize(&path)?;
    let response = OpenedWorkspace {
        path: workspace.path(),
        documents: workspace.list()?,
    };
    *slot = Some(workspace);
    Ok(response)
}
#[tauri::command]
pub async fn list_documents(
    state: State<'_, DesktopState>,
) -> Result<Vec<DocumentSummary>, String> {
    state
        .workspace
        .lock()
        .await
        .as_ref()
        .ok_or("工作流数据尚未就绪，请重试初始化")?
        .list()
}
#[tauri::command]
pub async fn load_document(
    id: String,
    state: State<'_, DesktopState>,
) -> Result<LoadedDocument, String> {
    state
        .workspace
        .lock()
        .await
        .as_ref()
        .ok_or("工作流数据尚未就绪，请重试初始化")?
        .load(&id)
}
#[tauri::command]
pub async fn save_document(
    file: WorkflowFile,
    revision: Option<String>,
    state: State<'_, DesktopState>,
) -> Result<LoadedDocument, String> {
    if state
        .runs
        .snapshot()
        .await
        .is_some_and(|run| run.status.is_active() && run.documents.contains(&file.id))
    {
        return Err("运行快照涉及的文档暂时只读".into());
    }
    state
        .workspace
        .lock()
        .await
        .as_ref()
        .ok_or("工作流数据尚未就绪，请重试初始化")?
        .save(&file, revision.as_deref())
}
#[tauri::command]
pub async fn delete_document(
    id: String,
    revision: String,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    if state
        .runs
        .snapshot()
        .await
        .is_some_and(|run| run.status.is_active() && run.documents.contains(&id))
    {
        return Err("运行快照涉及的文档暂时只读".into());
    }
    state
        .workspace
        .lock()
        .await
        .as_ref()
        .ok_or("工作流数据尚未就绪，请重试初始化")?
        .remove(&id, &revision)
}
#[tauri::command]
pub async fn validate_workflow(
    id: String,
    state: State<'_, DesktopState>,
) -> Result<Vec<argusflow_workflow::Diagnostic>, String> {
    let bundle = load_bundle(
        state
            .workspace
            .lock()
            .await
            .as_ref()
            .ok_or("工作流数据尚未就绪，请重试初始化")?,
        &id,
        &Default::default(),
    )?;
    state.runs.validate(bundle).await
}
#[tauri::command]
pub async fn start_workflow(
    id: String,
    inputs: serde_json::Value,
    revisions: std::collections::BTreeMap<String, String>,
    channel: Channel<RunMessage>,
    state: State<'_, DesktopState>,
) -> Result<String, String> {
    let bundle = load_bundle(
        state
            .workspace
            .lock()
            .await
            .as_ref()
            .ok_or("工作流数据尚未就绪，请重试初始化")?,
        &id,
        &revisions,
    )?;
    // 借助相同无损值边界解析运行输入，不接受浮点化的整数。
    let value = crate::document::wire::decode_inputs(inputs)?;
    state.runs.start(bundle, value, channel).await
}
#[tauri::command]
pub async fn stop_workflow(state: State<'_, DesktopState>) -> Result<(), String> {
    state.runs.cancel().await
}
#[tauri::command]
pub async fn capabilities(state: State<'_, DesktopState>) -> Result<Vec<String>, String> {
    state.runs.capabilities().await
}
#[tauri::command]
pub async fn shutdown_desktop(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.runs.shutdown().await?;
    app.exit(0);
    Ok(())
}
#[tauri::command]
pub fn parse_node_clipboard(
    source: String,
) -> Result<crate::document::clipboard::NodeClipboard, String> {
    crate::document::clipboard::parse(&source)
}
#[tauri::command]
pub async fn describe_task(
    type_id: String,
    config: serde_json::Value,
    state: State<'_, DesktopState>,
) -> Result<argusflow_workflow::Fields, String> {
    state.runs.task_inputs(type_id, config).await
}
#[tauri::command]
pub async fn get_run(
    channel: Channel<RunMessage>,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.runs.subscribe(channel).await;
    Ok(())
}
#[tauri::command]
pub fn export_log(path: String, text: String) -> Result<(), String> {
    if text.len() > 16 * 1024 * 1024 {
        return Err("日志超过导出大小限制".into());
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}
