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

/// 工作流对高风险系统能力的显式授权。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPermissions {
    /// 允许创建任意子进程；三种 CommandRunner 都需要。
    pub process_spawn: bool,
    /// 允许使用 PowerShell 语言运行器。
    pub powershell: bool,
    /// 允许使用 CMD shell 运行器。
    pub cmd: bool,
}

impl WorkflowPermissions {
    /// 创建只允许无 shell 直接启动程序的最小命令权限。
    pub const fn direct_process_only() -> Self {
        Self {
            process_spawn: true,
            powershell: false,
            cmd: false,
        }
    }
}
