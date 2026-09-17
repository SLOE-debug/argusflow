//! 桌面程序入口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--recorder-video")
    {
        if let Err(error) = argusflow_desktop::run_video_child() {
            eprintln!("视频进程退出：{error}");
            std::process::exit(2);
        }
        return;
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--recorder-child")
    {
        if let Err(error) = argusflow_desktop::run_child() {
            eprintln!("录制子进程退出：{error}");
            std::process::exit(2);
        }
        return;
    }
    argusflow_desktop::run();
}
