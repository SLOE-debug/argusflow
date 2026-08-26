use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

use crate::ValueExpr;

/// Command 节点选择的同语义命令运行器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRunner {
    /// 直接启动程序并逐项传递参数，不经过 shell。
    Direct,
    /// 使用非交互 PowerShell 执行脚本。
    PowerShell,
    /// 使用 Windows CMD 执行脚本。
    Cmd,
}

/// 工作流可以按最小粒度授权的开放系统能力标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowCapabilityId(String);

impl WorkflowCapabilityId {
    /// Application/Browser 资源节点启动新进程的能力。
    pub fn application_launch() -> Self {
        Self::new("process.application.launch")
    }

    /// Command 节点不经过 shell 直接启动程序的能力。
    pub fn direct_command() -> Self {
        Self::new("process.command.direct")
    }

    /// Command 节点使用 PowerShell 脚本运行器的能力。
    pub fn powershell() -> Self {
        Self::new("process.command.powershell")
    }

    /// Command 节点使用 CMD 脚本运行器的能力。
    pub fn cmd() -> Self {
        Self::new("process.command.cmd")
    }

    /// 创建由注册节点拥有的稳定权限能力 ID。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 返回持久化和诊断使用的稳定能力名称。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkflowCapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// 传给子进程的单个环境变量绑定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBinding {
    /// 环境变量名称；运行时拒绝空名称和包含 `=` 的名称。
    pub name: String,
    /// 在命令准备阶段解析并冻结的环境变量值。
    pub value: ValueExpr,
}

/// 统一描述 Direct、PowerShell 与 CMD 的命令执行语义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandOperation {
    /// 使用的命令运行器。
    pub runner: CommandRunner,
    /// Direct 模式的程序路径或名称；其它模式必须为空。
    pub program: Option<ValueExpr>,
    /// Direct 模式逐项传递的参数；其它模式必须为空。
    pub arguments: Vec<ValueExpr>,
    /// PowerShell/CMD 模式的脚本文本；Direct 模式必须为空。
    pub script: Option<ValueExpr>,
    /// 可选工作目录。
    pub working_directory: Option<ValueExpr>,
    /// 显式追加或覆盖的子进程环境变量。
    pub environment: Vec<EnvironmentBinding>,
    /// 可选标准输入文本。
    pub stdin: Option<ValueExpr>,
    /// 从启动到完整退出的最长毫秒数。
    pub timeout_ms: u64,
    /// 被视为成功的进程退出代码；必须至少包含一项。
    pub accepted_exit_codes: Vec<i32>,
    /// 保留 stdout 的最大字节数，超限会终止节点而不是静默截断。
    pub max_stdout_bytes: usize,
    /// 保留 stderr 的最大字节数，超限会终止节点而不是静默截断。
    pub max_stderr_bytes: usize,
}

/// 工作流对高风险系统能力的开放显式授权集合。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPermissions {
    /// 所有显式授权的稳定能力；未列出的能力一律拒绝。
    pub allow: BTreeSet<WorkflowCapabilityId>,
}

impl WorkflowPermissions {
    /// 创建只允许无 shell 直接启动程序的最小命令权限。
    pub fn direct_command_only() -> Self {
        Self {
            allow: BTreeSet::from([WorkflowCapabilityId::direct_command()]),
        }
    }

    /// 从宿主或编辑器显式授权的能力集合创建权限。
    pub fn from_iter(capabilities: impl IntoIterator<Item = WorkflowCapabilityId>) -> Self {
        Self {
            allow: capabilities.into_iter().collect(),
        }
    }

    /// 判断工作流是否声明了指定系统能力。
    pub fn allows(&self, capability: &WorkflowCapabilityId) -> bool {
        self.allow.contains(capability)
    }

    /// 判断权限是否明确允许所选命令运行器。
    pub fn allows_command(&self, runner: CommandRunner) -> bool {
        self.allows(&required_command_capability(runner))
    }
}

/// 返回所选命令运行器创建进程所需的唯一能力。
pub fn required_command_capability(runner: CommandRunner) -> WorkflowCapabilityId {
    match runner {
        CommandRunner::Direct => WorkflowCapabilityId::direct_command(),
        CommandRunner::PowerShell => WorkflowCapabilityId::powershell(),
        CommandRunner::Cmd => WorkflowCapabilityId::cmd(),
    }
}
