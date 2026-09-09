//! 录制命令只适配控制面与证据读取，不编译工作流。

use crate::runtime::AppState;
use argusflow_recorder::{CompletedRecording, RecorderStatus, RecordingSummary};
use tauri::State;

/// 显式开始全局真实输入录制；重复开始返回领域错误。
#[tauri::command]
pub(crate) async fn start_recording(
    state: State<'_, AppState>,
    privacy: argusflow_recorder::RecordingPrivacy,
) -> Result<RecorderStatus, String> {
    state
        .recorder
        .start_with_privacy(privacy)
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

/// 只识别已有事件截图，复用本地 Paddle OCR worker。
#[tauri::command]
pub(crate) async fn read_recording_screen_frame(
    state: State<'_, AppState>,
    recording_id: uuid::Uuid,
    frame_id: argusflow_recorder::ScreenFrameId,
    region_index: Option<u32>,
) -> Result<tauri::ipc::Response, String> {
    state
        .recorder
        .screen_region(recording_id, frame_id, region_index)
        .await
        .map(tauri::ipc::Response::new)
        .map_err(|error| error.to_string())
}

/// 只识别已有事件截图，复用本地 Paddle OCR worker。
#[tauri::command]
pub(crate) async fn recognize_recording_screenshot(
    state: State<'_, AppState>,
    recording_id: uuid::Uuid,
    sequence: u64,
    kind: argusflow_recorder::ScreenshotKind,
) -> Result<Vec<argusflow_vision::OcrItem>, String> {
    let engine = state
        .vision_pipe
        .as_ref()
        .ok_or("文字识别尚未就绪，请在应用初始化完成后重试。")?;
    let recording = state
        .recorder
        .load(recording_id)
        .await
        .map_err(|error| error.to_string())?;
    let window = recording
        .trace
        .timeline
        .events
        .iter()
        .find(|event| event.sequence == sequence)
        .and_then(|event| event.evidence.as_ref())
        .and_then(|evidence| evidence.context.as_ref())
        .map(|context| context.window)
        .ok_or("这张截图缺少窗口信息，请手动框选需要遮盖的区域。")?;
    let bytes = state
        .recorder
        .screenshot(recording_id, sequence, kind)
        .await
        .map_err(|error| error.to_string())?;
    argusflow_vision::recognize_saved_png(engine.as_ref(), bytes, window, sequence)
        .await
        .map_err(|_| "截图文字识别失败，请重试或手动框选区域。".into())
}

/// 批量执行事件抹除或区域遮盖，返回重新持久化的录制。
#[tauri::command]
pub(crate) async fn edit_recording_privacy(
    state: State<'_, AppState>,
    recording_id: uuid::Uuid,
    edits: Vec<argusflow_recorder::PrivacyEdit>,
) -> Result<CompletedRecording, String> {
    state
        .recorder
        .edit_privacy(recording_id, edits)
        .await
        .map_err(|error| error.to_string())
}
