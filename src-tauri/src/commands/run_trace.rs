//! 历史 Run Trace 的只读查询命令。

use argusflow_runtime::{RunDetails, RunManifest, RunTraceEvent};
use tauri::State;
use uuid::Uuid;

use crate::runtime::AppState;

/// 列出本机全部运行 Manifest，按开始时间倒序排列。
#[tauri::command]
pub fn list_runs(state: State<'_, AppState>) -> Result<Vec<RunManifest>, String> {
    state.run_store.list_runs()
}

/// 读取一次运行的 Manifest 与执行时工作流快照。
#[tauri::command]
pub fn get_run(state: State<'_, AppState>, run_id: Uuid) -> Result<RunDetails, String> {
    state.run_store.get_run(run_id)
}

/// 读取一次运行的完整 JSONL 事件 envelope。
#[tauri::command]
pub fn read_run_events(
    state: State<'_, AppState>,
    run_id: Uuid,
) -> Result<Vec<RunTraceEvent>, String> {
    state.run_store.read_events(run_id)
}

/// 返回安全 artifact 引用对应的原始二进制体；IPC 不进行 JSON byte-array 编码。
#[tauri::command]
pub fn read_run_artifact(
    state: State<'_, AppState>,
    run_id: Uuid,
    artifact_id: String,
) -> Result<tauri::ipc::Response, String> {
    state
        .run_store
        .read_artifact(run_id, &artifact_id)
        .map(tauri::ipc::Response::new)
}
