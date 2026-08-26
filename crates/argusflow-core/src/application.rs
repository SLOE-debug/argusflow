use serde::{Deserialize, Serialize};

use crate::WindowTitleMatcher;

/// 应用资源节点获取 direct-process Windows 桌面应用所需的完整契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationSpec {
    /// 进程身份匹配和直接启动共同使用的绝对 EXE 路径。
    pub executable_path: String,
    /// 不经过 shell 解析、直接传给 EXE 的参数列表。
    pub arguments: Vec<String>,
    /// 从同一 EXE 的顶层窗口中筛选唯一目标的标题规则。
    pub window_title: WindowTitleMatcher,
    /// 决定复用现有进程还是启动新进程的获取策略。
    pub acquire_policy: AcquirePolicy,
    /// 启动后等待可交互顶层窗口的最长毫秒数。
    pub launch_timeout_ms: u64,
    /// 工作流结束时应用进程的清理策略。
    pub cleanup_policy: CleanupPolicy,
    /// 获取会话时是否需要把窗口带到前台。
    pub activation_policy: ActivationPolicy,
}

/// 应用资源节点复用和启动进程的策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquirePolicy {
    /// 优先连接唯一现有应用，没有匹配项时直接启动。
    #[default]
    AttachOrStart,
    /// 只连接已经运行的唯一应用，不创建进程。
    AttachOnly,
    /// 无论现有实例如何都直接启动一个新进程，并只接受该进程的窗口。
    AlwaysStartNew,
}

impl AcquirePolicy {
    /// 判断该策略是否可能创建新的应用进程。
    pub const fn may_launch(self) -> bool {
        !matches!(self, Self::AttachOnly)
    }
}

/// 工作流结束时应用会话的资源回收策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPolicy {
    /// 不关闭应用，保留用户原有状态。
    #[default]
    LeaveRunning,
    /// 只关闭由本次工作流启动的应用。
    CloseIfStartedByWorkflow,
    /// 无论应用来源如何都尝试关闭匹配窗口。
    AlwaysClose,
}

/// 应用会话获取时的窗口激活要求。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationPolicy {
    /// 仅恢复窗口，不发起前台激活请求。
    None,
    /// 请求前台激活，但 Windows foreground lock 拒绝不会导致获取失败。
    #[default]
    BestEffort,
    /// 必须成功把窗口带到前台，否则应用节点失败。
    Required,
}
