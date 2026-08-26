use argusflow_core::{ExecutionEventKind, ExecutionEventPayload};

use crate::NodeOutcome;

/// PreparedNode 交给 Engine 发出的单个节点内事件。
#[derive(Debug)]
pub struct NodeEvent {
    /// 节点内事件类别。
    pub kind: ExecutionEventKind,
    /// 可选说明；不得包含未被节点语义明确允许记录的敏感数据。
    pub message: Option<String>,
    /// 可安全传给前端的结构化载荷。
    pub payload: Option<ExecutionEventPayload>,
}

/// 一个节点完成后的结构化结果与可观察事件。
#[derive(Debug, Default)]
pub struct NodeExecution {
    /// 保存到 RunContext 的值和资源端口。
    pub outcome: NodeOutcome,
    /// 在 NodeSucceeded 前按顺序发出的节点内事件。
    pub events: Vec<NodeEvent>,
}
