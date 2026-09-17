//! 本地录制配额与剩余空间检查；不自动清理历史会话。
use super::model::{Result, VideoError};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows::{Win32::Storage::FileSystem::GetDiskFreeSpaceExW, core::HSTRING};
pub(super) struct StorageGuard {
    directory: PathBuf,
    max_bytes: u64,
    checked: Instant,
}
impl StorageGuard {
    pub fn new(directory: &Path, max_bytes: u64) -> Result<Self> {
        let guard = Self {
            directory: directory.to_path_buf(),
            max_bytes,
            checked: Instant::now(),
        };
        guard.check()?;
        Ok(guard)
    }
    pub fn tick(&mut self) -> Result<()> {
        if self.checked.elapsed() >= Duration::from_secs(1) {
            self.check()?;
            self.checked = Instant::now();
        }
        Ok(())
    }
    fn check(&self) -> Result<()> {
        let mut total = 0u64;
        for entry in std::fs::read_dir(&self.directory)? {
            let metadata = entry?.metadata()?;
            if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
        if total >= self.max_bytes {
            return Err(VideoError::Invalid(
                "录制达到本地文件配额，正在结束已有前缀".into(),
            ));
        }
        let mut free = 0;
        // SAFETY: 调用只读取目录所属卷的可用空间，指针指向有效局部变量。
        unsafe {
            GetDiskFreeSpaceExW(
                &HSTRING::from(self.directory.as_os_str()),
                Some(&mut free),
                None,
                None,
            )?;
        }
        if free < 256 * 1024 * 1024 {
            return Err(VideoError::Invalid(
                "磁盘剩余空间不足256MiB，停止录制".into(),
            ));
        }
        Ok(())
    }
}
