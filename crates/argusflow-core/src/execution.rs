use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 工作流启动后返回给调用方的运行标识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStarted {
    /// 本次运行的唯一标识。
    pub run_id: Uuid,
}

/// 运行时按序发出的工作流执行事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// 本次运行的唯一标识。
    pub run_id: Uuid,
    /// 事件所属工作流的唯一标识。
    pub workflow_id: Uuid,
    /// 从零开始、在单次运行内严格递增的事件序号。
    pub sequence: u64,
    /// 相关节点 ID；工作流级事件不绑定具体节点。
    pub node_id: Option<String>,
    /// 事件的生命周期类别。
    pub kind: ExecutionEventKind,
    /// 可选的人类可读消息或错误摘要。
    pub message: Option<String>,
}

/// 工作流和节点生命周期中可观察的事件类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventKind {
    /// 工作流已开始执行。
    WorkflowStarted,
    /// 某个节点已开始执行。
    NodeStarted,
    /// 节点产生了日志消息。
    Log,
    /// 某个节点执行成功。
    NodeSucceeded,
    /// 某个节点执行失败。
    NodeFailed,
    /// 工作流中的所有节点已完成。
    WorkflowCompleted,
    /// 工作流因节点或运行时错误而失败。
    WorkflowFailed,
}
