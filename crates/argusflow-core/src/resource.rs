use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::ApplicationSpec;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Planner 可以从应用会话使用的执行能力事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppCapabilities {
    /// 会话窗口可通过 Windows UI Automation 查询。
    pub windows_uia: bool,
    /// 会话已经具备可附加的浏览器调试上下文。
    pub browser_cdp: bool,
    /// 会话可以使用屏幕视觉定位。
    pub visual: bool,
    /// 应用具有保持业务语义的专用命令适配器。
    pub command_adapter: bool,
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
    pub capabilities: AppCapabilities,
    /// 当前进程是否由本次工作流获取动作启动。
    pub started_by_workflow: bool,
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

/// 平台应用运行时向 WorkflowEngine 提供的资源生命周期边界。
#[async_trait]
pub trait ApplicationSessionProvider: Send + Sync {
    /// 获取、恢复或启动满足契约的应用，并返回运行时会话。
    async fn acquire(&self, spec: &ApplicationSpec) -> Result<AppSession, ApplicationError>;

    /// 按会话冻结的 CleanupPolicy 回收资源。
    async fn cleanup(&self, session: &AppSession) -> Result<(), ApplicationError>;
}
