//! ArgusFlow Tauri 应用入口及其命令、运行时状态编排。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod commands;
mod run_trace_sink;
mod runtime;
mod startup;
mod window_bootstrap;

use tauri::Manager;

/// 构建并启动 ArgusFlow 的 Tauri 应用。
///
/// 返回 Tauri 启动过程中的错误；工作流运行时状态由应用启动时统一注入。
pub fn run() -> tauri::Result<()> {
    let app_state = runtime::AppState::new();
    let app = tauri::Builder::default()
        .manage(app_state)
        .plugin(window_bootstrap::init())
        .invoke_handler(tauri::generate_handler![
            commands::startup::begin_runtime_initialization,
            commands::startup::get_startup_status,
            commands::startup::retry_startup,
            commands::query::inspect_aql,
            commands::run_trace::list_runs,
            commands::run_trace::get_run,
            commands::run_trace::read_run_events,
            commands::run_trace::read_run_artifact,
            commands::workflow::validate_workflow,
            commands::workflow::run_workflow,
        ])
        .build(tauri::generate_context!())?;
    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Err(error) = app_handle.state::<runtime::AppState>().shutdown() {
                eprintln!("ArgusFlow capture host shutdown failed: {error}");
            }
        }
    });
    Ok(())
}
