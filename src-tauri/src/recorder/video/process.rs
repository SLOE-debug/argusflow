//! 监督视频原生调用；超时只终止自己创建的子进程。
use std::{
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
pub(crate) struct VideoProcess {
    child: Child,
    directory: PathBuf,
}
impl VideoProcess {
    pub async fn start(recording: &Path) -> Result<Self, String> {
        let root = recording.join("video");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut used = 0u64;
        let mut count = 0u32;
        for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            count += 1;
            for file in std::fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                used = used.saturating_add(
                    file.map_err(|e| e.to_string())?
                        .metadata()
                        .map_err(|e| e.to_string())?
                        .len(),
                );
            }
        }
        if count >= 256 || used >= 4 * 1024 * 1024 * 1024 - 256 * 1024 * 1024 {
            return Err("录制达到视频片段或4GiB容量预算，请开始新的录制".into());
        }
        let directory = root.join(format!("{:06}", count + 1));
        let error_file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join(format!("{:06}.stderr", count + 1)))
            .map_err(|e| e.to_string())?;
        let child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--recorder-video")
            .arg(&directory)
            .arg((4 * 1024 * 1024 * 1024 - used).to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(error_file)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut owned = Self { child, directory };
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            owned.check()?;
            if owned.directory.join("ready.json").exists() {
                return Ok(owned);
            }
            if Instant::now() >= deadline {
                return Err("视频采集未在6秒内就绪".into());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    pub fn check(&mut self) -> Result<(), String> {
        if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
            let path = self.directory.with_extension("stderr");
            let error = std::fs::read_to_string(path).unwrap_or_default();
            return Err(format!(
                "视频录制进程结束（{status}）：{}",
                error.chars().take(1000).collect::<String>()
            ));
        }
        Ok(())
    }
    pub async fn stop(mut self) -> Result<(), String> {
        if let Some(mut pipe) = self.child.stdin.take() {
            let _ = pipe.write_all(b"S");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                if status.success() && self.directory.join("complete.json").exists() {
                    return Ok(());
                }
                return Err("视频片段未完整结束；保留已写入前缀".into());
            }
            if Instant::now() >= deadline {
                return Err("视频收尾超过5秒，已终止子进程并保留已有文件".into());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}
impl Drop for VideoProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
