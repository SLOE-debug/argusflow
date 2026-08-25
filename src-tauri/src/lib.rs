//! ArgusFlow Tauri 应用入口及其命令、运行时状态编排。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod commands;
mod runtime;

/// 构建并启动 ArgusFlow 的 Tauri 应用。
///
/// 返回 Tauri 启动过程中的错误；工作流运行时状态由应用启动时统一注入。
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .manage(runtime::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::query::inspect_aql,
            commands::workflow::validate_workflow,
            commands::workflow::run_workflow,
        ])
        .run(tauri::generate_context!())
}
