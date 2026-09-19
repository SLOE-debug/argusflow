//! 命令只装配配置存储、正式录制适配器和实际任务注册表。
use super::{
    AiJobs,
    result::{DraftResult, into_draft},
};
use argusflow_ai::{AiConfig, ConfigStore, ConfigView, Evidence, Progress, SaveConfig};
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State, ipc::Channel};
fn database(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("settings.sqlite3"))
}
#[tauri::command]
pub(crate) async fn ai_config(app: tauri::AppHandle) -> Result<ConfigView, String> {
    let path = database(&app)?;
    tokio::task::spawn_blocking(move || ConfigStore::open(&path)?.view())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) async fn ai_save_config(
    app: tauri::AppHandle,
    update: SaveConfig,
) -> Result<ConfigView, String> {
    let path = database(&app)?;
    tokio::task::spawn_blocking(move || ConfigStore::open(&path)?.save(update))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) fn ai_cancel(id: String, jobs: State<'_, AiJobs>) -> Result<(), String> {
    jobs.cancel(&id)
}
#[tauri::command]
pub(crate) async fn ai_analyze(
    app: tauri::AppHandle,
    id: String,
    directory: String,
    channel: Channel<Progress>,
    jobs: State<'_, AiJobs>,
) -> Result<DraftResult, String> {
    if id.is_empty() || id.len() > 128 {
        return Err("分析请求 ID 无效".into());
    }
    let job = jobs.begin(id)?;
    let database = database(&app)?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings");
    let loaded = tokio::task::spawn_blocking(move || {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let directory = PathBuf::from(directory)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if directory.parent() != Some(root.as_path()) {
            return Err("只能整理本机录制历史".into());
        }
        let (config, key) = ConfigStore::open(&database)
            .and_then(|s| s.load())
            .map_err(|e| e.to_string())?;
        let evidence = Evidence::load(&directory).map_err(|e| e.to_string())?;
        Ok::<(AiConfig, String, Evidence), String>((config, key, evidence))
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut registry = argusflow_runtime::NodeRegistry::new();
    // 仅编译，无 UIA、剪贴板或浏览器运行服务，不会执行副作用。
    argusflow_workflow_automation::register_automation(&mut registry, Default::default())?;
    let session_id = loaded.2.session_id().to_owned();
    let result = argusflow_ai::analyze(
        loaded.0,
        loaded.1,
        Arc::new(loaded.2),
        &registry,
        job.token.clone(),
        |event| {
            let _ = channel.send(event);
        },
    )
    .await
    .map_err(|e| e.to_string())?;
    if job.token.is_cancelled() {
        return Err("AI 整理已取消".into());
    }
    into_draft(result, &session_id)
}
