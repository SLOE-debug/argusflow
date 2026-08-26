use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ApplicationSpec, CommandOperation, ConditionPredicate, UiOperation, ValueExpr,
    WorkflowInputDefinition, WorkflowPermissions,
};

/// 可序列化的完整工作流定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// 当前持久化契约版本；运行时会拒绝不支持的版本。
    pub schema_version: u32,
    /// 工作流的稳定唯一标识。
    pub id: Uuid,
    /// 面向用户显示的工作流名称。
    pub name: String,
    /// 本工作流引用的瞬时运行输入声明；不保存任何一次运行的实际值。
    pub inputs: Vec<WorkflowInputDefinition>,
    /// 条件节点读取的只读 JSON 变量；根值必须是对象。
    pub variables: Value,
    /// 对进程和 shell 等高风险能力的显式授权。
    pub permissions: WorkflowPermissions,
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
    /// 显式把一个运行时文本值写入调试日志。
    Debug {
        /// 在节点执行时解析并展示的文本表达式。
        value: ValueExpr,
    },
    /// 在继续执行下一节点前等待指定时长。
    Delay {
        /// 暂停时长，单位为毫秒；运行时校验范围为 1 到 60000。
        milliseconds: u64,
    },
    /// 根据结构化谓词选择 True 或 False 分支。
    Condition {
        /// 在只读工作流变量上执行的安全条件。
        predicate: ConditionPredicate,
    },
    /// 获取一个可被后续界面节点复用的应用会话资源。
    Application {
        /// 应用身份、获取策略和生命周期策略。
        spec: ApplicationSpec,
    },
    /// 将语义界面操作交给 Planner 选择等价后端执行。
    Ui {
        /// 包含资源作用域和数据表达式的界面操作。
        operation: UiOperation,
    },
    /// 执行 Direct、PowerShell 或 CMD 命令并产生结构化输出。
    Command {
        /// 命令运行方式、输入输出边界和超时。
        operation: CommandOperation,
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
    /// Condition 源节点的分支标签；其他节点的连线必须为空。
    pub branch: Option<ConditionBranch>,
}

/// Condition 节点的两个互斥出口。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionBranch {
    /// 条件成立时选择的出口。
    True,
    /// 条件不成立时选择的出口。
    False,
}
