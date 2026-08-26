use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{WorkflowInputDefinition, WorkflowPermissions};

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
    /// 开放节点定义；序列化时类型、版本和 payload 会被展开到节点对象中。
    #[serde(flatten)]
    pub definition: NodeEnvelope,
}

/// 编辑器画布中的二维位置。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// 水平坐标。
    pub x: f64,
    /// 垂直坐标。
    pub y: f64,
}

/// Definition registry 使用的稳定开放节点类型标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeTypeId(String);

impl NodeTypeId {
    /// 创建由注册提供器拥有的稳定节点类型 ID。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 返回持久化和注册表查找使用的完整类型名称。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 工作流持久化层中的开放节点定义。
///
/// 动态 JSON 只存在于加载与编译边界。Runtime 必须先通过 `NodeTypeRegistry` 把它
/// 解码为强类型 `PreparedNode`，执行热路径不会反复查 schema 或读取 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeEnvelope {
    /// 指向唯一注册编译器的稳定节点类型。
    pub type_id: NodeTypeId,
    /// 该节点类型自己的 payload 契约版本，与工作流 schema 独立演进。
    pub version: u16,
    /// 仅供对应类型编译器解码的节点参数。
    pub payload: Value,
}

impl NodeEnvelope {
    /// 从已经构造的 JSON payload 创建开放节点定义。
    pub fn new(type_id: impl Into<String>, version: u16, payload: Value) -> Self {
        Self {
            type_id: NodeTypeId::new(type_id),
            version,
            payload,
        }
    }

    /// 从可序列化的强类型 payload 创建持久化定义。
    pub fn from_payload<T>(
        type_id: impl Into<String>,
        version: u16,
        payload: &T,
    ) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        Ok(Self {
            type_id: NodeTypeId::new(type_id),
            version,
            payload: serde_json::to_value(payload)?,
        })
    }
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
    pub branch: Option<ControlPortId>,
}

/// 分支节点公开的开放控制流端口标识。
///
/// 内置 Condition 使用 `true`/`false`，注册节点可以声明任意稳定端口集合，Validator
/// 会根据 PreparedNode 的端口描述检查边，而不需要修改中央枚举。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlPortId(String);

impl ControlPortId {
    /// 创建由节点类型拥有的稳定控制流端口。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 返回边契约和注册节点共享的稳定端口名称。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
