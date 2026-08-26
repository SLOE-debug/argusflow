use std::collections::{BTreeMap, HashMap};

use argusflow_core::{AppSession, ResourceId, ResourceRef, ValueExpr};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::RuntimeError;

/// 一个工作流节点成功后保存在运行上下文中的结构化结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOutcome {
    /// 可被后续 ValueExpr 引用的值输出。
    pub outputs: BTreeMap<String, Value>,
    /// 本次节点产生的逻辑资源输出名称。
    pub resources: Vec<String>,
}

impl NodeOutcome {
    /// 创建只包含值输出的节点结果。
    pub fn values(outputs: BTreeMap<String, Value>) -> Self {
        Self {
            outputs,
            resources: Vec::new(),
        }
    }
}

/// Runtime 当前支持的真实资源类别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceEntry {
    /// Windows 桌面应用逻辑会话。
    Application(AppSession),
}

/// 单次运行独占的真实资源与逻辑引用绑定表。
#[derive(Debug, Default)]
pub struct ResourceTable {
    /// 按运行时 ID 保存真实资源，避免把 OS 身份放进工作流 JSON。
    resources: HashMap<ResourceId, ResourceEntry>,
    /// 将生产节点输出端口绑定到运行时资源 ID。
    bindings: HashMap<ResourceRef, ResourceId>,
    /// 按获取顺序记录资源，工作流结束时反向清理。
    acquisition_order: Vec<ResourceId>,
}

impl ResourceTable {
    /// 绑定一个 Application 节点的 `session` 输出。
    pub fn insert_application(&mut self, reference: ResourceRef, session: AppSession) {
        let resource_id = session.id;
        self.resources
            .insert(resource_id, ResourceEntry::Application(session));
        self.bindings.insert(reference, resource_id);
        self.acquisition_order.push(resource_id);
    }

    /// 解析逻辑引用并要求其绑定应用会话。
    pub fn application(&self, reference: &ResourceRef) -> Result<&AppSession, RuntimeError> {
        let resource_id =
            self.bindings
                .get(reference)
                .ok_or_else(|| RuntimeError::ResourceUnavailable {
                    reference: reference.clone(),
                })?;
        match self.resources.get(resource_id) {
            Some(ResourceEntry::Application(session)) => Ok(session),
            None => Err(RuntimeError::ResourceUnavailable {
                reference: reference.clone(),
            }),
        }
    }

    /// 返回所有应用会话的反向获取顺序副本，供异步清理时避免跨 await 借用表。
    pub fn applications_for_cleanup(&self) -> Vec<AppSession> {
        self.acquisition_order
            .iter()
            .rev()
            .filter_map(|resource_id| match self.resources.get(resource_id) {
                Some(ResourceEntry::Application(session)) => Some(session.clone()),
                None => None,
            })
            .collect()
    }
}

/// 单次运行的数据面、节点输出和资源生命周期状态。
#[derive(Debug)]
pub struct RunContext {
    /// 本次运行的稳定 ID。
    pub run_id: Uuid,
    /// 启动时冻结的只读输入对象。
    workflow_inputs: Map<String, Value>,
    /// 已成功执行节点的结构化结果。
    node_outputs: HashMap<String, NodeOutcome>,
    /// 本次运行独占的真实资源表。
    resources: ResourceTable,
    /// 可由未来变量节点更新的运行内变量存储。
    variables: Map<String, Value>,
}

impl RunContext {
    /// 从独立的运行输入和工作流初始变量创建运行上下文。
    pub fn new(
        run_id: Uuid,
        workflow_inputs: Map<String, Value>,
        variables: Map<String, Value>,
    ) -> Self {
        Self {
            run_id,
            workflow_inputs,
            node_outputs: HashMap::new(),
            resources: ResourceTable::default(),
            variables,
        }
    }

    /// 返回当前资源表的只读视图。
    pub const fn resources(&self) -> &ResourceTable {
        &self.resources
    }

    /// 返回当前资源表的可变视图，仅供资源节点绑定输出。
    pub const fn resources_mut(&mut self) -> &mut ResourceTable {
        &mut self.resources
    }

    /// 保存一个成功节点的结果，覆盖同一节点的旧结果以支持未来重试语义。
    pub fn record_outcome(&mut self, node_id: String, outcome: NodeOutcome) {
        self.node_outputs.insert(node_id, outcome);
    }

    /// 解析 ValueExpr，并克隆出不依赖上下文借用的冻结 JSON 值。
    pub fn resolve_value(&self, expression: &ValueExpr) -> Result<Value, RuntimeError> {
        match expression {
            ValueExpr::Literal { value } => Ok(value.clone()),
            ValueExpr::WorkflowInput { key } => {
                self.workflow_inputs.get(key).cloned().ok_or_else(|| {
                    RuntimeError::ValueUnavailable {
                        description: format!("workflow input '{key}' is unavailable"),
                    }
                })
            }
            ValueExpr::NodeOutput { node_id, output } => self
                .node_outputs
                .get(node_id)
                .and_then(|outcome| outcome.outputs.get(output))
                .cloned()
                .ok_or_else(|| RuntimeError::ValueUnavailable {
                    description: format!("node output '{node_id}.{output}' is unavailable"),
                }),
            ValueExpr::Variable { name } => {
                self.variables
                    .get(name)
                    .cloned()
                    .ok_or_else(|| RuntimeError::ValueUnavailable {
                        description: format!("runtime variable '{name}' is unavailable"),
                    })
            }
        }
    }

    /// 解析必须为字符串的节点参数，不对数字或布尔值做隐式转换。
    pub fn resolve_text(&self, expression: &ValueExpr) -> Result<String, RuntimeError> {
        self.resolve_value(expression)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| RuntimeError::ValueTypeMismatch { expected: "string" })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use argusflow_core::ValueExpr;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{NodeOutcome, RunContext};

    #[test]
    fn resolves_inputs_variables_and_prior_node_outputs_without_coercion() {
        let inputs = json!({ "order_id": "ACME-10086" })
            .as_object()
            .expect("fixture inputs should be an object")
            .clone();
        let variables = json!({ "region": "east" })
            .as_object()
            .expect("fixture variables should be an object")
            .clone();
        let mut context = RunContext::new(Uuid::new_v4(), inputs, variables);
        context.record_outcome(
            "read-order".to_owned(),
            NodeOutcome::values(BTreeMap::from([(
                "text".to_owned(),
                Value::String("ACME-10086".to_owned()),
            )])),
        );

        assert_eq!(
            context
                .resolve_text(&ValueExpr::WorkflowInput {
                    key: "order_id".to_owned(),
                })
                .expect("workflow input should resolve"),
            "ACME-10086",
        );
        assert_eq!(
            context
                .resolve_text(&ValueExpr::Variable {
                    name: "region".to_owned(),
                })
                .expect("initial runtime variable should resolve"),
            "east",
        );
        assert_eq!(
            context
                .resolve_text(&ValueExpr::NodeOutput {
                    node_id: "read-order".to_owned(),
                    output: "text".to_owned(),
                })
                .expect("prior node output should resolve"),
            "ACME-10086",
        );
    }
}
