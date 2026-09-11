//! 先由页面提交编辑草稿，再通过命令等待引擎收尾。
use tauri::{Emitter, Window, WindowEvent};

pub fn handle_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if let Err(error) = window.emit("desktop-close-requested", ()) {
            eprintln!("无法通知页面保存草稿：{error}");
        }
    }
}
