//! 前端启动页读取与重试桌面能力的 Tauri 命令。

use tauri::{AppHandle, State};

use crate::{
    runtime::AppState,
    startup::{StartupSnapshot, start_status_publisher},
};

/// 由 React 启动页提交后调用，立即触发后台能力初始化。
#[tauri::command]
pub fn begin_runtime_initialization(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> StartupSnapshot {
    if state.start_capabilities() {
        start_status_publisher(app_handle);
    }
    current_snapshot(&state)
}

/// 返回当前 WGC 与 OCR 分阶段启动快照，不触发任何初始化或 I/O。
#[tauri::command]
pub fn get_startup_status(state: State<'_, AppState>) -> StartupSnapshot {
    current_snapshot(&state)
}

/// 重试失败能力；命令只触发后台初始化，不等待模型预热完成。
#[tauri::command]
pub fn retry_startup(app_handle: AppHandle, state: State<'_, AppState>) -> StartupSnapshot {
    if state.start_capabilities() {
        start_status_publisher(app_handle);
    } else {
        state.retry_capabilities();
    }
    current_snapshot(&state)
}

/// 从共享状态构造无 I/O 的当前启动快照。
fn current_snapshot(state: &AppState) -> StartupSnapshot {
    StartupSnapshot::from_health(state.vision_health(), state.startup_elapsed())
}
