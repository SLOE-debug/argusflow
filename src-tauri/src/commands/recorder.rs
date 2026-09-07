//! 录制命令只适配控制面与证据读取，不编译工作流。

use crate::runtime::AppState;
use argusflow_recorder::{CompletedRecording, RecorderStatus, RecordingSummary};
use tauri::State;

/// 显式开始全局真实输入录制；重复开始返回领域错误。
#[tauri::command]
pub(crate) async fn start_recording(state: State<'_, AppState>) -> Result<RecorderStatus, String> {
    state
        .recorder
        .start()
        .await
        .map_err(|error| error.to_string())
}

/// 停止并排空 worker，返回事件时间线与本地证据目录。
#[tauri::command]
pub(crate) async fn stop_recording(
    state: State<'_, AppState>,
) -> Result<CompletedRecording, String> {
    state
        .recorder
        .stop()
        .await
        .map_err(|error| error.to_string())
}

/// 读取录制生命周期和丢失事件计数。
#[tauri::command]
pub(crate) async fn get_recording_status(
    state: State<'_, AppState>,
) -> Result<RecorderStatus, String> {
    Ok(state.recorder.status().await)
}

/// 返回已保存录制的摘要，不传输输入内容。
#[tauri::command]
pub(crate) async fn list_recordings(
    state: State<'_, AppState>,
) -> Result<Vec<RecordingSummary>, String> {
    state
        .recorder
        .list()
        .await
        .map_err(|error| error.to_string())
}

/// 从录制根目录按强类型 ID 读取已脱敏 Trace。
#[tauri::command]
pub(crate) async fn get_recording(
    state: State<'_, AppState>,
    recording_id: uuid::Uuid,
) -> Result<CompletedRecording, String> {
    state
        .recorder
        .load(recording_id)
        .await
        .map_err(|error| error.to_string())
}

/// 按录制 ID 与事件序号读取 PNG，不接受文件路径。
#[tauri::command]
pub(crate) async fn read_recording_screenshot(
    state: State<'_, AppState>,
    recording_id: uuid::Uuid,
    sequence: u64,
    kind: argusflow_recorder::ScreenshotKind,
) -> Result<tauri::ipc::Response, String> {
    state
        .recorder
        .screenshot(recording_id, sequence, kind)
        .await
        .map(tauri::ipc::Response::new)
        .map_err(|error| error.to_string())
}
