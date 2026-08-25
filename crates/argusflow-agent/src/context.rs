use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// 当前前台窗口的稳定规划信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowContext {
    /// 原生窗口句柄的无符号表示。
    pub handle: u64,
    /// 窗口所属进程 ID。
    pub process_id: u32,
}

/// 当前活动进程的稳定规划信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessContext {
    /// 操作系统进程 ID。
    pub process_id: u32,
    /// 不包含路径的可执行文件名。
    pub executable_name: String,
}

/// 当前浏览器调试会话上下文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserSessionContext {
    /// 当前 CDP target 的稳定 ID。
    pub target_id: String,
    /// 会话是否仍处于 attached 状态。
    pub attached: bool,
}

/// Windows Accessibility API 的运行状态。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessibilityContext {
    /// 当前进程是否已初始化 UI Automation apartment 与 client。
    pub ready: bool,
}

/// 视觉缓存的运行状态。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualCacheContext {
    /// 当前前台画面是否有可复用的感知缓存。
    pub ready: bool,
}

/// Planner 每次准备动作时使用的不可变运行环境快照。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// 当前前台窗口。
    pub foreground_window: Option<WindowContext>,
    /// 当前活动进程。
    pub active_process: Option<ProcessContext>,
    /// 当前可用浏览器会话。
    pub browser_session: Option<BrowserSessionContext>,
    /// Accessibility 子系统状态。
    pub accessibility: AccessibilityContext,
    /// 视觉缓存状态。
    pub visual_cache: VisualCacheContext,
}

/// Backend 与当前上下文的匹配程度，位于 availability 之后、cost 之前参与排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFitness {
    /// 上下文明确指向该 backend，例如 attached Chrome target。
    Excellent,
    /// 上下文完整且适合该 backend。
    Good,
    /// 没有足够信息区分候选。
    Neutral,
    /// backend 可用，但与当前上下文不匹配。
    Poor,
}

impl ContextFitness {
    /// 返回从优到劣的排序序号。
    pub const fn rank(self) -> u8 {
        match self {
            Self::Excellent => 0,
            Self::Good => 1,
            Self::Neutral => 2,
            Self::Poor => 3,
        }
    }
}

/// 为路由器提供每次执行所需的最新上下文快照。
pub trait ExecutionContextProvider: Send + Sync {
    /// 捕获当前不可变上下文。
    fn snapshot(&self) -> ExecutionContext;
}

/// 可由宿主更新的线程安全上下文提供器。
#[derive(Debug, Default)]
pub struct StaticExecutionContext {
    /// 当前上下文；锁只保护短时 clone，不跨 await 持有。
    context: RwLock<ExecutionContext>,
}

impl StaticExecutionContext {
    /// 以给定初始上下文创建提供器。
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            context: RwLock::new(context),
        }
    }

    /// 原子替换宿主维护的上下文快照。
    pub fn replace(&self, context: ExecutionContext) {
        *self
            .context
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = context;
    }
}

impl ExecutionContextProvider for StaticExecutionContext {
    fn snapshot(&self) -> ExecutionContext {
        self.context
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}
