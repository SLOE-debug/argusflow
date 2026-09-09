//! CDP 连接预算与显式浏览器启动选项。
use crate::BrowserError as Failure;
use argusflow_core::FailureKind;
use std::{path::PathBuf, time::Duration};

/// CDP 连接资源预算。
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// 排队和等待响应合计最多 128 个请求。
    pub max_in_flight: usize,
    /// 单帧消息的最大字节数，默认 8 MiB。
    pub max_message_bytes: usize,
    /// 一次 WebSocket 写入的最长时间。
    pub write_timeout: Duration,
    /// 同一连接最多附加的页面数。
    pub max_pages: usize,
    /// 一次 CSS 查询最多返回的元素数量。
    pub max_elements: usize,
}
impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            max_in_flight: 128,
            max_message_bytes: 8 * 1024 * 1024,
            write_timeout: Duration::from_secs(5),
            max_pages: 64,
            max_elements: 256,
        }
    }
}
impl BrowserConfig {
    pub(super) fn validate(&self) -> Result<(), Failure> {
        if self.max_in_flight == 0
            || self.max_in_flight > 4096
            || self.max_message_bytes < 1024
            || self.max_message_bytes > 64 * 1024 * 1024
            || self.write_timeout.is_zero()
            || self.write_timeout > Duration::from_secs(60)
            || self.max_pages == 0
            || self.max_pages > 1024
            || self.max_elements == 0
            || self.max_elements > 4096
        {
            Err(Failure::new(
                FailureKind::InvalidInput,
                "browser_config",
                "浏览器配置超出范围",
            ))
        } else {
            Ok(())
        }
    }
}

/// 浏览器启动选项，不读取或复用用户的日常浏览器配置目录。
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Chrome/Edge 可执行文件的绝对路径。
    pub executable: PathBuf,
    /// 是否启用 Chromium headless 模式，默认 false。
    pub headless: bool,
    /// 启动总时限，默认 30 秒。
    pub timeout: Duration,
}
impl LaunchOptions {
    /// 使用默认可见模式和 30 秒时限。
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            headless: false,
            timeout: Duration::from_secs(30),
        }
    }
}
