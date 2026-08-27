use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ValueExpr, WorkflowEdge, WorkflowInputDefinition, WorkflowNode};

/// 可复用流程组件的稳定标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FlowComponentId(Uuid);

impl FlowComponentId {
    /// 从稳定 UUID 创建组件标识。
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// 返回序列化契约使用的 UUID。
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// 必须精确锁定的组件发布版本。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FlowComponentVersion(String);

impl FlowComponentVersion {
    /// 创建版本值；注册表会在接收定义时校验 `major.minor.patch` 格式。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 返回持久化和注册表查找使用的精确版本文本。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 组件公开的一个具名值输出。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentValueOutput {
    /// 组件实例公开的非空唯一端口名。
    pub name: String,
    /// 在内部出口前已经可用的值表达式。
    pub value: ValueExpr,
}

/// 可版本化、可嵌套的流程组件定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowComponentDefinition {
    /// 当前组件持久化契约版本。
    pub schema_version: u32,
    /// 跨名称和版本保持不变的组件标识。
    pub id: FlowComponentId,
    /// 实例必须精确锁定的发布版本。
    pub version: FlowComponentVersion,
    /// 面向用户显示的组件名称。
    pub name: String,
    /// 内部图可以通过 `workflow_input` 引用的显式值输入。
    pub inputs: Vec<WorkflowInputDefinition>,
    /// 组件实例对父工作流公开的显式值输出。
    pub outputs: Vec<ComponentValueOutput>,
    /// 组件内部节点图；必须包含唯一边界 Start 与 End。
    pub nodes: Vec<WorkflowNode>,
    /// 组件内部有向边。
    pub edges: Vec<WorkflowEdge>,
    /// 唯一入口边界节点 ID。
    pub entry_node_id: String,
    /// 唯一出口边界节点 ID。
    pub exit_node_id: String,
}

/// 主流程中精确锁定一个组件版本的实例 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentInstance {
    /// 要解析的稳定组件标识。
    pub component_id: FlowComponentId,
    /// 不允许隐式漂移的精确发布版本。
    pub component_version: FlowComponentVersion,
    /// 按组件输入名绑定父工作流 ValueExpr。
    pub inputs: BTreeMap<String, ValueExpr>,
}
