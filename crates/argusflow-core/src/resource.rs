use std::{borrow::Cow, collections::BTreeSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{ApplicationSpec, BrowserSpec};

/// 工作流定义中指向资源输出端口的逻辑引用。
///
/// 引用不包含 PID、HWND 或其它平台句柄；真实资源只存在于单次运行的 ResourceTable。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    /// 产生资源的节点 ID。
    pub producer_node_id: String,
    /// 生产节点公开的资源输出端口名称。
    pub output_name: String,
}

/// 单次运行内分配给真实资源的稳定标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceId(Uuid);

impl ResourceId {
    /// 为新获取的运行时资源生成标识。
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

/// 工作流资源端口和运行时资源实例共享的开放类型标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceTypeId(String);

impl ResourceTypeId {
    /// Windows 桌面应用会话资源。
    pub fn application() -> Self {
        Self::new("argus.application.session")
    }
    /// Chromium CDP 页面会话资源。
    pub fn browser() -> Self {
        Self::new("argus.browser.session")
    }

    /// 从运行时注册名称创建资源类型 ID。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 返回持久化契约和注册表使用的稳定名称。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 应用会话绑定的进程身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    /// Windows 进程 ID。
    pub process_id: u32,
    /// 规范化后的完整可执行文件路径。
    pub executable_path: String,
}

/// 应用会话当前可用的顶层窗口身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowIdentity {
    /// 原生 HWND 的无符号不透明表示。
    pub handle: u64,
    /// 窗口所属进程 ID，用于检测句柄复用。
    pub process_id: u32,
}

/// 可由资源提供器和 Planner 共同识别的开放能力标识。
///
/// 内置能力使用静态字符串且无需分配；外部注册能力可以持有自己的名称。能力 ID
/// 只描述事实，不隐含后端优先级或可用性。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId(Cow<'static, str>);

impl CapabilityId {
    /// Windows UI Automation 查询与动作能力。
    pub const WINDOWS_UIA: Self = Self::from_static("ui.windows.uia");
    /// Chromium DevTools Protocol 页面能力。
    pub const BROWSER_CDP: Self = Self::from_static("browser.cdp");
    /// 屏幕视觉定位能力。
    pub const VISUAL_SCREEN: Self = Self::from_static("vision.screen");
    /// 保持业务语义的应用命令适配能力。
    pub const COMMAND_ADAPTER: Self = Self::from_static("command.adapter");

    /// 从编译期稳定名称创建零分配能力 ID。
    pub const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }

    /// 从运行时注册名称创建能力 ID。
    pub fn new(value: impl Into<String>) -> Self {
        Self(Cow::Owned(value.into()))
    }

    /// 返回用于注册、Explain 和诊断的稳定名称。
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

/// 应用会话公开给 Planner 的开放能力集合。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    /// 有序集合保证 Explain、日志和测试输出稳定。
    capabilities: BTreeSet<CapabilityId>,
}

impl CapabilitySet {
    /// 从资源提供器确认的能力事实创建集合。
    pub fn from_iter(capabilities: impl IntoIterator<Item = CapabilityId>) -> Self {
        Self {
            capabilities: capabilities.into_iter().collect(),
        }
    }

    /// 判断资源是否公开指定能力。
    pub fn contains(&self, capability: &CapabilityId) -> bool {
        self.capabilities.contains(capability)
    }

    /// 返回稳定有序且只读的能力迭代器。
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CapabilityId> {
        self.capabilities.iter()
    }
}

/// 单次运行持有的逻辑应用会话。
///
/// 会话保存可重新验证的进程与窗口身份，不把任何 COM 对象或拥有所有权的 OS handle
/// 放进可序列化工作流定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSession {
    /// ResourceTable 使用的运行时资源 ID。
    pub id: ResourceId,
    /// 获取该会话时冻结的应用契约和生命周期策略。
    pub spec: ApplicationSpec,
    /// 当前匹配应用的进程身份。
    pub process: ProcessIdentity,
    /// 当前匹配且已经恢复的顶层窗口。
    pub windows: Vec<WindowIdentity>,
    /// Planner 可消费的运行时能力事实。
    pub capabilities: CapabilitySet,
    /// 当前进程是否由本次工作流获取动作启动。
    pub started_by_workflow: bool,
}

/// 单次运行持有的隔离 Chromium 页面会话。
///
/// 工作流只保存可重新关联 CDP runtime 注册项的资源 ID，不持有 WebSocket 或进程句柄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSession {
    /// ResourceTable 使用的运行时资源 ID。
    pub id: ResourceId,
    /// 获取阶段冻结的浏览器启动契约。
    pub spec: BrowserSpec,
    /// 本次隔离浏览器的根进程 ID。
    pub process_id: u32,
    /// 当前页面的稳定 CDP target ID。
    pub target_id: String,
}

/// Windows 应用资源获取或回收失败。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplicationError {
    /// 应用定义无法形成确定且安全的获取请求。
    #[error("invalid application specification: {message}")]
    InvalidSpec {
        /// 无效字段或约束说明。
        message: String,
    },
    /// AttachOnly 没有找到匹配应用。
    #[error("application is not running: {description}")]
    NotRunning {
        /// 不包含敏感输出的应用描述。
        description: String,
    },
    /// 多个应用窗口满足同一身份约束，不能隐式选择。
    #[error("application matched {matches} windows and requires a unique target: {description}")]
    Ambiguous {
        /// 匹配的窗口数量。
        matches: usize,
        /// 不包含句柄的应用描述。
        description: String,
    },
    /// 启动或观察应用进程失败。
    #[error("application launch failed: {message}")]
    LaunchFailed {
        /// 失败原因。
        message: String,
    },
    /// 应用没有在配置的时间边界内产生目标窗口。
    #[error("application acquisition timed out after {timeout_ms} ms: {description}")]
    Timeout {
        /// 配置的等待时长。
        timeout_ms: u64,
        /// 不包含敏感输出的应用描述。
        description: String,
    },
    /// Required 激活策略无法把目标窗口带到前台。
    #[error("application window activation failed: {message}")]
    ActivationFailed {
        /// 激活失败原因。
        message: String,
    },
    /// 按清理策略关闭应用失败。
    #[error("application cleanup failed: {message}")]
    CleanupFailed {
        /// 清理失败原因。
        message: String,
    },
}

/// Chromium 浏览器会话获取或回收失败。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BrowserError {
    /// 浏览器定义无法形成确定且安全的启动请求。
    #[error("invalid browser specification: {message}")]
    InvalidSpec {
        /// 无效字段或约束说明。
        message: String,
    },
    /// 浏览器进程、调试端点或 WebSocket 初始化失败。
    #[error("browser launch failed: {message}")]
    LaunchFailed {
        /// 不包含页面内容的失败原因。
        message: String,
    },
    /// 浏览器没有在配置时限内公开可用页面 target。
    #[error("browser acquisition timed out after {timeout_ms} ms")]
    Timeout {
        /// 配置的完整等待时长。
        timeout_ms: u64,
    },
    /// 浏览器会话关闭或临时配置清理失败。
    #[error("browser cleanup failed: {message}")]
    CleanupFailed {
        /// 清理失败说明。
        message: String,
    },
}

/// 平台应用运行时向 WorkflowEngine 提供的资源生命周期边界。
#[async_trait]
pub trait ApplicationSessionProvider: Send + Sync {
    /// 获取、恢复或启动满足契约的应用，并返回运行时会话。
    async fn acquire(&self, spec: &ApplicationSpec) -> Result<AppSession, ApplicationError>;

    /// 按会话冻结的 CleanupPolicy 回收资源。
    async fn cleanup(&self, session: &AppSession) -> Result<(), ApplicationError>;
}

/// 浏览器运行时向 WorkflowEngine 提供的隔离会话生命周期边界。
#[async_trait]
pub trait BrowserSessionProvider: Send + Sync {
    /// 启动浏览器、连接随机 CDP 端口并返回页面会话资源。
    async fn acquire(&self, spec: &BrowserSpec) -> Result<BrowserSession, BrowserError>;

    /// 关闭 CDP 会话、浏览器进程并清理本次隔离用户目录。
    async fn cleanup(&self, session: &BrowserSession) -> Result<(), BrowserError>;
}
