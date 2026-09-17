//! ArgusFlow 桌面应用装配入口。
mod commands;
pub mod document;
mod recorder;
mod runtime;
mod startup;
mod window;
use commands::*;
pub use recorder::{run_child, run_video_child};

/// 启动桌面工作台。
pub fn run() {
    let result = tauri::Builder::default()
        .manage(startup::StartupTiming::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(DesktopState::default())
        .setup(|app| {
            startup::native_ready(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            startup::frontend_ready,
            recorder::recorder_start,
            recorder::recorder_status,
            recorder::recorder_transition,
            recorder::recorder_list,
            recorder::recorder_session,
            recorder::recorder_read,
            recorder::recorder_context,
            recorder::recorder_image,
            recorder::recorder_video_frame,
            recorder::recorder_video_timeline,
            initialize_workspace,
            list_documents,
            load_document,
            save_document,
            delete_document,
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
