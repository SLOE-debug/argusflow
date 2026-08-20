use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AutomationAction;

/// 可序列化的完整工作流定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// 当前持久化契约版本；运行时会拒绝不支持的版本。
    pub schema_version: u32,
    /// 工作流的稳定唯一标识。
    pub id: Uuid,
    /// 面向用户显示的工作流名称。
    pub name: String,
    /// 按节点定义执行内容及画布位置。
    pub nodes: Vec<WorkflowNode>,
    /// 描述节点之间执行顺序的有向连线。
    pub edges: Vec<WorkflowEdge>,
}

/// 工作流画布中的一个节点。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowNode {
    /// 节点标识；同一工作流内必须唯一。
    pub id: String,
    /// 节点在编辑器画布中的位置，单位由客户端画布约定。
    pub position: Position,
    /// 节点的可执行类型及其参数；序列化时字段会被展开到节点对象中。
    #[serde(flatten)]
    pub kind: WorkflowNodeKind,
}

/// 编辑器画布中的二维位置。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// 水平坐标。
    pub x: f64,
    /// 垂直坐标。
    pub y: f64,
}

/// 节点的具体执行语义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowNodeKind {
    /// 线性执行链的唯一入口节点。
    Start,
    /// 向执行事件流写入一条消息。
    Log {
        /// 执行时写入事件流的消息。
        message: String,
    },
    /// 在继续执行下一节点前等待指定时长。
    Delay {
        /// 暂停时长，单位为毫秒；运行时校验范围为 1 到 60000。
        milliseconds: u64,
    },
    /// 将自动化操作交给匹配的后端执行。
    Action {
        /// 要交给自动化后端执行的动作。
        action: AutomationAction,
    },
    /// 线性执行链的唯一出口节点。
    End,
}

/// 描述一个节点到下一个节点的有向连线。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    /// 连线标识；同一工作流内必须唯一。
    pub id: String,
    /// 起始节点 ID。
    pub source: String,
    /// 目标节点 ID。
    pub target: String,
}
