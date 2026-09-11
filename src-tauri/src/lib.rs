//! ArgusFlow 桌面应用装配入口。
mod commands;
pub mod document;
mod runtime;
mod window;
use commands::*;

/// 启动桌面工作台。
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(DesktopState::default())
        .invoke_handler(tauri::generate_handler![
            initialize_workspace,
            list_documents,
            load_document,
            save_document,
            validate_workflow,
            start_workflow,
            stop_workflow,
            get_run,
            export_log,
            describe_task,
            shutdown_desktop,
            parse_node_clipboard,
            capabilities
        ])
        .on_window_event(window::handle_event)
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("无法启动 ArgusFlow 桌面应用：{error}");
        std::process::exit(1);
    }
}
