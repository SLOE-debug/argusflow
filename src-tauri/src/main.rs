#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! ArgusFlow Tauri 进程入口，负责启动应用库并报告不可恢复的启动错误。

fn main() {
    if let Err(error) = argusflow_lib::run() {
        eprintln!("ArgusFlow exited with an error: {error}");
        std::process::exit(1);
    }
}
