//! 同一打包EXE的视频模式；管道EOF立即请求停止。
use argusflow_windows::{VideoOptions, record_desktop_video_until};
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
/// 打包EXE的受监督视频模式，不创建应用窗口。
pub fn run_video_child() -> Result<(), String> {
    let directory = PathBuf::from(std::env::args_os().nth(2).ok_or("缺少视频片段目录")?);
    let quota: u64 = std::env::args()
        .nth(3)
        .ok_or("缺少视频配额")?
        .parse()
        .map_err(|_| "视频配额无效")?;
    let stop = Arc::new(AtomicBool::new(false));
    let requested = stop.clone();
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0u8; 1]);
        requested.store(true, Ordering::Release);
        // 父进程崩溃后没有监督者可终止卡死的驱动；本进程只给自己有限收尾时间。
        std::thread::sleep(Duration::from_secs(5));
        std::process::exit(3);
    });
    record_desktop_video_until(
        VideoOptions {
            directory,
            adapter: 0,
            output: 0,
            fps: 30,
            bitrate: 8_000_000,
            max_file_bytes: quota,
            duration: Duration::from_secs(3600),
        },
        &stop,
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}
