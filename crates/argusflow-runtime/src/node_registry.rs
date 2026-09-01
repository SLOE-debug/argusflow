use std::{collections::BTreeMap, fmt, sync::Arc};

use argusflow_core::{
    ControlPortId, NodeEnvelope, NodeTypeId, ResourceRef, ResourceTypeId, ValueExpr,
    WorkflowDefinition, WorkflowNode, WorkflowPermissions,
};
use async_trait::async_trait;

use crate::{
    AccessSet, NodeExecution, RunContext, RuntimeError, ValidationIssue, ValidationIssueCode,
};

/// 节点对条件 DAG 公开的控制流形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFlow {
    /// 唯一入口；无入边且恰好一条出边。
    Start,
    /// 普通节点；至少一条入边且恰好一条出边。
    Linear,
    /// 多分支节点；每个已声明端口必须有且只有一条出边。
    Branch {
        /// 该节点类型拥有的稳定开放控制流端口。
        ports: Vec<ControlPortId>,
    },
    /// 拥有独立子作用域的有界 While 容器。
    Loop {
        /// 必须包含 `completed` 与 `exhausted` 的稳定父图端口集合。
        ports: Vec<ControlPortId>,
        /// 该容器唯一拥有的子作用域。
        body_scope_id: String,
        /// 单次激活最多开始的轮次数。
        max_iterations: u32,
        /// 单次激活的总毫秒预算。
        timeout_ms: u64,
        /// 第二轮起的间隔毫秒数。
        interval_ms: u64,
    },
    /// While 子作用域每轮的固定入口。
    LoopEntry,
    /// While 子作用域请求开始下一轮的固定出口。
    LoopContinue,
    /// While 子作用域请求正常完成容器的固定出口。
    LoopComplete,
    /// 唯一出口；至少一条入边且没有出边。
    End,
}

/// 节点值端口和数据消费者共享的开放类型标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueTypeId(String);

impl ValueTypeId {
    /// 创建由注册节点拥有的稳定值类型 ID。
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 当前 ValueExpr 文本参数接受的内置字符串类型。
    pub fn text() -> Self {
        Self::new("argus.value.text")
    }

    /// 不带更具体 schema 的结构化 JSON 类型。
    pub fn json() -> Self {
        Self::new("argus.value.json")
    }

    /// JSON number 类型。
    pub fn number() -> Self {
        Self::new("argus.value.number")
    }

    /// JSON boolean 类型。
    pub fn boolean() -> Self {
        Self::new("argus.value.boolean")
    }

    /// 返回注册表和诊断使用的稳定值类型名称。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// PreparedNode 声明的强类型资源输入。
#[derive(Debug, Clone, Copy)]
pub struct ResourceInput<'node> {
    /// 工作流定义中的逻辑资源引用。
    pub reference: &'node ResourceRef,
    /// 生产端口必须公开的资源类型。
    pub expected_type: &'node ResourceTypeId,
}

/// PreparedNode 声明的强类型值输入。
#[derive(Debug, Clone)]
pub struct ValueInput<'node> {
    /// 工作流定义中的值表达式。
    pub expression: &'node ValueExpr,
    /// 生产端口或字面量必须满足的开放值类型。
    pub expected_type: ValueTypeId,
}

impl<'node> ValueInput<'node> {
    /// 创建要求指定开放值类型的输入声明。
    pub fn new(expression: &'node ValueExpr, expected_type: ValueTypeId) -> Self {
        Self {
            expression,
            expected_type,
        }
    }

    /// 创建要求文本值的内置输入声明。
    pub fn text(expression: &'node ValueExpr) -> Self {
        Self::new(expression, ValueTypeId::text())
    }

    /// 创建接受任意普通 JSON 值的内置输入声明。
    pub fn json(expression: &'node ValueExpr) -> Self {
        Self::new(expression, ValueTypeId::json())
    }
}

/// 节点自身参数校验可读取的不可变工作流上下文。
pub struct NodeValidationContext<'workflow> {
    /// 完整工作流定义，供权限、变量和输入声明校验使用。
    pub workflow: &'workflow WorkflowDefinition,
    /// 当前节点 ID，供问题精确定位。
    pub node_id: &'workflow str,
}

impl NodeValidationContext<'_> {
    /// 创建定位到当前节点的结构化校验问题。
    pub fn issue(&self, code: ValidationIssueCode, message: impl Into<String>) -> ValidationIssue {
        ValidationIssue {
            code,
            message: message.into(),
            node_id: Some(self.node_id.to_owned()),
            edge_id: None,
            scope_id: None,
            structure_path: Vec::new(),
        }
    }
}

/// 已在 prepare 阶段完成 JSON 解码的强类型节点执行对象。
///
/// trait object 只承担节点级分派；实现内部保留具体 payload，执行时不再访问
/// `NodeEnvelope.payload` 或动态 schema 注册表。
#[async_trait]
pub trait PreparedNode: fmt::Debug + Send + Sync {
    /// 节点的控制流形状。
    fn flow(&self) -> NodeFlow;

    /// 返回不会泄露业务输入或敏感数据的事件摘要。
    fn label(&self) -> String;

    /// 校验已解码 payload 中依赖工作流级上下文的约束。
    fn validate(&self, _context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        Vec::new()
    }

    /// 返回当前节点消费的全部值表达式。
    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        Vec::new()
    }

    /// 返回当前节点消费的全部强类型资源引用。
    fn resource_inputs(&self) -> Vec<ResourceInput<'_>> {
        Vec::new()
    }

    /// 查询当前节点是否公开指定值输出端口。
    fn value_output(&self, _name: &str) -> Option<ValueTypeId> {
        None
    }

    /// 查询当前节点是否公开指定资源输出端口。
    fn resource_output(&self, _name: &str) -> Option<&ResourceTypeId> {
        None
    }

    /// 根据当前 RunWorld 中已绑定的资源身份解析调度访问集合。
    fn access_set(&self, _node_id: &str, _context: &RunContext) -> Result<AccessSet, RuntimeError> {
        Ok(AccessSet::default())
    }

    /// 当前节点是否在成功执行时绑定需要生命周期清理的资源。
    fn acquires_resources(&self) -> bool {
        false
    }

    /// 执行冻结的强类型节点计划并更新单次运行上下文。
    async fn execute(
        &self,
        node_id: &str,
        permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError>;
}

/// 一个开放节点类型在 definition/prepare 边界的编译器。
pub trait NodeCompiler: Send + Sync {
    /// 返回该编译器唯一处理的稳定类型 ID。
    fn type_id(&self) -> &NodeTypeId;

    /// 校验版本并把动态 payload 一次性解码为强类型节点。
    fn compile(&self, definition: &NodeEnvelope)
    -> Result<Arc<dyn PreparedNode>, NodeCompileError>;
}

/// 节点 payload 无法被注册编译器冻结时的定义错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeCompileError {
    /// 不包含业务 payload 的安全错误说明。
    pub message: String,
}

impl NodeCompileError {
    /// 创建一个面向验证报告的节点编译错误。
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// 按稳定类型 ID 查找节点编译器的开放注册表。
#[derive(Default)]
pub struct NodeTypeRegistry {
    /// 每种类型最多存在一个编译器，BTreeMap 保证诊断顺序稳定。
    compilers: BTreeMap<NodeTypeId, Arc<dyn NodeCompiler>>,
}

impl NodeTypeRegistry {
    /// 创建空注册表，供宿主按功能模块装配。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个节点编译器；重复类型 ID 会被明确拒绝。
    pub fn register(&mut self, compiler: Arc<dyn NodeCompiler>) -> Result<(), NodeRegistryError> {
        let type_id = compiler.type_id().clone();
        if self.compilers.contains_key(&type_id) {
            return Err(NodeRegistryError::DuplicateType { type_id });
        }
        self.compilers.insert(type_id, compiler);
        Ok(())
    }

    /// 由运行时内置模块装配一组类型 ID 已在源码中证明唯一的编译器。
    pub(crate) fn from_builtin_compilers(
        compilers: impl IntoIterator<Item = Arc<dyn NodeCompiler>>,
    ) -> Self {
        let compilers = compilers
            .into_iter()
            .map(|compiler| (compiler.type_id().clone(), compiler))
            .collect();
        Self { compilers }
    }

    /// 把单个开放定义编译为强类型冻结节点。
    pub(crate) fn compile(
        &self,
        node: &WorkflowNode,
    ) -> Result<Arc<dyn PreparedNode>, NodeRegistryError> {
        let type_id = &node.definition.type_id;
        let compiler =
            self.compilers
                .get(type_id)
                .ok_or_else(|| NodeRegistryError::UnknownType {
                    type_id: type_id.clone(),
                })?;
        compiler
            .compile(&node.definition)
            .map_err(|source| NodeRegistryError::InvalidDefinition {
                type_id: type_id.clone(),
                source,
            })
    }
}

/// 注册表装配或节点定义编译失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeRegistryError {
    /// 两个功能模块声明了同一个类型 ID。
    DuplicateType {
        /// 冲突的稳定节点类型。
        type_id: NodeTypeId,
    },
    /// 工作流引用了宿主没有注册的节点类型。
    UnknownType {
        /// 缺失的稳定节点类型。
        type_id: NodeTypeId,
    },
    /// 对应编译器拒绝了版本或 payload。
    InvalidDefinition {
        /// 产生错误的稳定节点类型。
        type_id: NodeTypeId,
        /// 编译器提供的安全原因。
        source: NodeCompileError,
    },
}

impl fmt::Display for NodeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateType { type_id } => {
                write!(
                    formatter,
                    "node type '{}' is already registered",
                    type_id.as_str()
                )
            }
            Self::UnknownType { type_id } => {
                write!(
                    formatter,
                    "node type '{}' is not registered",
                    type_id.as_str()
                )
            }
            Self::InvalidDefinition { type_id, source } => write!(
                formatter,
                "node type '{}' could not be compiled: {}",
                type_id.as_str(),
                source.message,
            ),
        }
    }
}

impl std::error::Error for NodeRegistryError {}
