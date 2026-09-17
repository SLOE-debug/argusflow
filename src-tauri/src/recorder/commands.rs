//! 录制 Tauri 命令仅负责参数和阻塞边界适配。
use super::{
    messages::*,
    reader::{self, RecordingEntry},
};
use crate::commands::DesktopState;
use argusflow_recorder::*;
use tauri::{Manager, State};
/// 时间线按原始事件身份读取关联控件证据。
#[tauri::command]
pub async fn recorder_context(
    directory: String,
    raw_id: u64,
) -> Result<super::context::OperationContext, String> {
    static READING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let permit = tokio::time::timeout(std::time::Duration::from_secs(10), READING.acquire())
        .await
        .map_err(|_| "读取控件信息超时，请重新选择操作")?
        .map_err(|_| "控件信息读取服务已关闭")?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        super::context::read(std::path::Path::new(&directory), raw_id)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings"))
}
#[tauri::command]
pub async fn recorder_start(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<String, String> {
    state.recorder.start(root(&app)?, &state.runs).await
}
#[tauri::command]
pub async fn recorder_status(state: State<'_, DesktopState>) -> Result<RecorderStatus, String> {
    Ok(state.recorder.status().await)
}
#[tauri::command]
pub async fn recorder_transition(
    state: State<'_, DesktopState>,
    phase: SessionPhase,
) -> Result<(), String> {
    state.recorder.transition(phase).await
}
#[tauri::command]
pub async fn recorder_list(app: tauri::AppHandle) -> Result<Vec<RecordingEntry>, String> {
    let root = root(&app)?;
    tokio::task::spawn_blocking(move || reader::list(&root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recorder_session(
    app: tauri::AppHandle,
    directory: String,
    state: State<'_, DesktopState>,
) -> Result<Session, String> {
    let local_root = root(&app)?.canonicalize().map_err(|e| e.to_string())?;
    let local_directory = std::path::Path::new(&directory)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if local_directory.parent() != Some(local_root.as_path()) {
        return Err("只能查看本机录制历史".into());
    }
    let session = load_session(std::path::Path::new(&directory)).map_err(|e| e.to_string())?;
    let status = state.recorder.status().await;
    if status.session.as_ref() != Some(&session.id)
        || matches!(
            status.phase,
            Some(SessionPhase::Stopped | SessionPhase::Faulted) | None
        )
    {
        tokio::task::spawn_blocking(move || {
            recover(
                std::path::Path::new(&directory),
                argusflow_windows::listening::qpc(),
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    Ok(session)
}
#[tauri::command]
pub async fn recorder_read(directory: String, cursor: Cursor) -> Result<JournalPage, String> {
    tokio::task::spawn_blocking(move || {
        read_page(std::path::Path::new(&directory), cursor, 128).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recorder_image(directory: String, image: Attachment) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        use base64::Engine;
        let bytes =
            read_attachment(std::path::Path::new(&directory), &image).map_err(|e| e.to_string())?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}
