//! 独立进程原型入口，便于父进程设置超时并采集资源指标。
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use argusflow_windows::{VideoOptions, record_desktop_video};
    let mut args = std::env::args().skip(1);
    let directory = args.next().ok_or("需要新输出目录")?.into();
    let seconds: u64 = args.next().unwrap_or_else(|| "10".into()).parse()?;
    let report = record_desktop_video(VideoOptions {
        directory,
        adapter: 0,
        output: 0,
        fps: 30,
        bitrate: 8_000_000,
        max_file_bytes: 4 * 1024 * 1024 * 1024,
        duration: std::time::Duration::from_secs(seconds),
    })?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("原型需要 Windows");
}
