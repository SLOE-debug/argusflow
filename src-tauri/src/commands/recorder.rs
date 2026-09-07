//! 录制命令只适配控制面；Hook、解析、脱敏和 normalization 均位于独立 crate。

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

/// 停止并排空 worker，返回已分离持久化的 Raw/Semantic Trace。
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
