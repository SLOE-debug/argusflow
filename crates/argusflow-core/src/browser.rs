use serde::{Deserialize, Serialize};

/// 受 ArgusFlow 管理的 Chromium 浏览器启动契约。
///
/// 每次获取都创建隔离的临时用户目录和随机 CDP 端口，不附加用户日常浏览器配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSpec {
    /// Chromium 系浏览器可执行文件的绝对路径。
    pub executable_path: String,
    /// 新浏览器页面首次导航的绝对 HTTP(S) URL。
    pub initial_url: String,
    /// 浏览器进程启动并公开 CDP target 的最长等待毫秒数。
    pub launch_timeout_ms: u64,
}
