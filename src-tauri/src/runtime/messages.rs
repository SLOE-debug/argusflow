//! 脱离原生对象的有序 IPC 消息。
use argusflow_runtime::{EventKind, ExecutionEvent, ExecutionLocation, RunError, RunStatus};
use serde::Serialize;

/// 工作台生命周期；终态只随完整结果快照发布。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopRunStatus {
    /// 执行业务节点。
    Running,
    /// 等待资源清理和日志消费。
    Cleaning,
    /// 执行与收尾完成。
    Completed,
    /// 业务、收尾或结果编码失败。
    Failed,
    /// 用户取消。
    Cancelled,
    /// 执行超时。
    TimedOut,
}
impl DesktopRunStatus {
    /// 此状态下运行快照关联的文档仍然只读。
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Cleaning)
    }
}
impl From<RunStatus> for DesktopRunStatus {
    fn from(value: RunStatus) -> Self {
        match value {
            RunStatus::Running => Self::Running,
            RunStatus::Cleaning => Self::Cleaning,
            RunStatus::Completed => Self::Completed,
            RunStatus::Failed => Self::Failed,
            RunStatus::Cancelled => Self::Cancelled,
            RunStatus::TimedOut => Self::TimedOut,
        }
    }
}

/// 日志定位信息；计数通过字符串无损传输。
#[derive(Clone, Serialize)]
pub struct Location {
    /// 独立工作流身份。
    pub workflow: Option<String>,
    /// 作用域身份。
    pub scope: String,
    /// 节点身份。
    pub node: Option<String>,
    /// 当前作用域激活实例。
    pub instance: String,
    /// 节点执行序号。
    pub execution: Option<String>,
}
/// 一条有界日志，不自动包含输入和变量值。
#[derive(Clone, Serialize)]
pub struct LogEntry {
    /// 本次运行事件序号。
    pub sequence: String,
    /// 从启动开始的毫秒数。
    pub elapsed_ms: String,
    /// 事件类别。
    pub kind: String,
    /// 日志级别。
    pub level: String,
    /// 可读消息。
    pub message: String,
    /// 根到节点的路径。
    pub path: Vec<Location>,
}
/// 可独立查询的运行状态，订阅丢失不影响最终结果。
#[derive(Clone, Serialize)]
pub struct RunSnapshot {
    /// 引擎运行 ID。
    pub id: String,
    /// 根文档身份。
    pub workflow: String,
    /// 本次冻结涉及的全部文档。
    pub documents: Vec<String>,
    /// 当前生命周期状态。
    pub status: DesktopRunStatus,
    /// 有界日志尾部。
    pub logs: Vec<LogEntry>,
    /// 最近尾部之外的截断数量。
    pub omitted: usize,
    /// 成功输出或空对象。
    pub outputs: serde_json::Value,
    /// 最终首错及附加收尾错误。
    pub errors: Vec<String>,
}
/// Channel 更新；客户端按运行 ID 和日志序号去重。
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunMessage {
    /// 初始订阅或最终结果的完整状态。
    Snapshot { snapshot: RunSnapshot },
    /// 增量日志。
    Log { id: String, entry: LogEntry },
    /// 收尾中的状态。
    Status {
        id: String,
        status: DesktopRunStatus,
    },
}
pub(super) fn locations(path: &[ExecutionLocation]) -> Vec<Location> {
    path.iter()
        .map(|item| Location {
            workflow: item.workflow.as_ref().map(|id| id.0.clone()),
            scope: item.scope.clone(),
            node: item.node.clone(),
            instance: item.instance.to_string(),
            execution: item.execution.map(|n| n.to_string()),
        })
        .collect()
}
pub(super) fn event(value: ExecutionEvent, elapsed: u128) -> LogEntry {
    let (kind, message, level) = match value.kind {
        EventKind::RunStarted => ("run_started", "开始运行".into(), "info"),
        EventKind::ScopeEntered => ("scope_entered", "进入作用域".into(), "info"),
        EventKind::NodeStarted => ("node_started", "开始执行".into(), "info"),
        EventKind::NodeCompleted => ("node_completed", "执行完成".into(), "info"),
        EventKind::NodeFailed => ("node_failed", "节点失败，正在中断并收尾".into(), "error"),
        EventKind::Retrying { attempt } => {
            ("retrying", format!("第 {attempt} 次安全尝试"), "warning")
        }
        EventKind::ScopeExited => ("scope_exited", "离开作用域".into(), "info"),
        EventKind::RunFinished => ("run_finished", "运行结束".into(), "info"),
    };
    LogEntry {
        sequence: value.sequence.to_string(),
        elapsed_ms: elapsed.to_string(),
        kind: kind.into(),
        level: level.into(),
        message,
        path: locations(&value.path),
    }
}
pub(super) fn errors(error: &RunError) -> Vec<String> {
    let mut result = vec![format!("{:?}：{}", error.kind, error.message)];
    for secondary in &error.secondary {
        result.extend(errors(secondary));
    }
    result
}
