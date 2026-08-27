use serde::{Deserialize, Serialize};

use crate::{ResourceRef, ValueExpr};

/// 受管浏览器资源当前支持的获取方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAcquireMode {
    /// 每次运行创建隔离 profile 与随机 CDP 端口。
    LaunchIsolatedCdp,
}

/// 工作流结束时对受管浏览器资源采用的清理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserCleanupPolicy {
    /// 关闭本次工作流启动的浏览器并删除隔离 profile。
    CloseOnWorkflowEnd,
}

/// 只负责获取受管 Chromium 会话的浏览器资源契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquireBrowserSpec {
    /// Chromium 系浏览器可执行文件的绝对路径。
    pub executable_path: String,
    /// 创建或连接浏览器会话的强类型策略。
    pub acquire_mode: BrowserAcquireMode,
    /// 浏览器进程启动并公开 CDP target 的最长等待毫秒数。
    pub launch_timeout_ms: u64,
    /// 工作流资源回收阶段采用的清理策略。
    pub cleanup_policy: BrowserCleanupPolicy,
}

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

/// 在已获取 BrowserSession 上执行的浏览器语义操作。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserOperation {
    /// 把当前页面导航到运行时解析得到的绝对 HTTP(S) URL。
    Navigate {
        /// 指向 Acquire Browser 节点的 `session` 资源输出。
        browser: ResourceRef,
        /// 运行前解析的目标地址。
        url: ValueExpr,
    },
}
