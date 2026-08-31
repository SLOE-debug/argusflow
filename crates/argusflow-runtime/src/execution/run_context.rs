use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_core::{ValueExpr, ValueSource};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    error::RuntimeError,
    resource::ResourceTable,
    value_runtime::{
        RuntimeValuePlan, RuntimeValueScope, evaluate_expression, validate_json_pointer,
    },
};

/// 一个工作流节点成功后保存在运行上下文中的结构化结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOutcome {
    /// 可被后续 ValueExpr 引用的 Published Outputs。
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

/// 单次运行的数据面、节点输出和资源生命周期状态。
#[derive(Debug)]
pub struct RunContext {
    /// 本次运行的稳定 ID。
    pub run_id: Uuid,
    /// 启动时冻结的只读输入对象。
    workflow_inputs: Map<String, Value>,
    /// 已成功执行节点的 Published Outputs 与资源端口。
    node_outputs: HashMap<String, NodeOutcome>,
    /// 本次运行独占的真实资源表。
    resources: ResourceTable,
    /// 可由变量节点事务式更新的运行内 JSON 变量存储。
    variables: Map<String, Value>,
    /// prepare 阶段编译、所有运行只读共享的表达式计划。
    value_plan: Arc<RuntimeValuePlan>,
    /// 每个 Loop Gate 独立维护的单调时钟与已开始轮次。
    loop_states: HashMap<String, LoopState>,
}

/// 单个有界循环在本次 RunWorld 中的瞬时状态。
#[derive(Debug)]
struct LoopState {
    /// 首次进入 Gate 的单调时钟时间。
    started_at: Instant,
    /// 已经进入循环体的轮次数。
    iterations: u32,
}

impl RunContext {
    /// 从独立的运行输入和工作流初始变量创建没有高级表达式的运行上下文。
    pub fn new(
        run_id: Uuid,
        workflow_inputs: Map<String, Value>,
        variables: Map<String, Value>,
    ) -> Self {
        Self::with_value_plan(
            run_id,
            workflow_inputs,
            variables,
            RuntimeValuePlan::empty(),
        )
    }

    /// 从 prepare 后的共享表达式计划创建 RunWorld 数据面。
    pub(crate) fn with_value_plan(
        run_id: Uuid,
        workflow_inputs: Map<String, Value>,
        variables: Map<String, Value>,
        value_plan: Arc<RuntimeValuePlan>,
    ) -> Self {
        Self {
            run_id,
            workflow_inputs,
            node_outputs: HashMap::new(),
            resources: ResourceTable::default(),
            variables,
            value_plan,
            loop_states: HashMap::new(),
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

    /// 保存一个成功节点的结果，覆盖同一节点的旧结果以支持重试语义。
    pub fn record_outcome(&mut self, node_id: String, outcome: NodeOutcome) {
        self.node_outputs.insert(node_id, outcome);
    }

    /// 返回指定 Gate 已经开始的轮次数；尚未进入时为零。
    pub(crate) fn loop_iterations(&self, node_id: &str) -> u32 {
        self.loop_states
            .get(node_id)
            .map_or(0, |state| state.iterations)
    }

    /// 在次数与总时长预算内开始下一轮；耗尽时返回已完成轮次。
    pub(crate) fn begin_loop_iteration(
        &mut self,
        node_id: &str,
        max_iterations: u32,
        timeout: Duration,
    ) -> Result<u32, u32> {
        let state = self
            .loop_states
            .entry(node_id.to_owned())
            .or_insert_with(|| LoopState {
                started_at: Instant::now(),
                iterations: 0,
            });
        if state.iterations >= max_iterations || state.started_at.elapsed() >= timeout {
            return Err(state.iterations);
        }
        state.iterations += 1;
        Ok(state.iterations)
    }

    /// 解析 ValueExpr，并克隆出不依赖上下文借用的冻结 JSON 值。
    pub fn resolve_value(&self, expression: &ValueExpr) -> Result<Value, RuntimeError> {
        let scope = self.value_scope(None);
        self.resolve_in_scope(expression, &scope)
    }

    /// 解析可能缺失的结构化引用；其它表达式错误仍然必须显式返回。
    pub fn resolve_optional_value(
        &self,
        expression: &ValueExpr,
    ) -> Result<Option<Value>, RuntimeError> {
        match self.resolve_value(expression) {
            Ok(value) => Ok(Some(value)),
            Err(RuntimeError::ValuePointerNotFound { .. })
            | Err(RuntimeError::ValueUnavailable { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// 解析必须为字符串的节点参数，不对数字或布尔值做隐式转换。
    pub fn resolve_text(&self, expression: &ValueExpr) -> Result<String, RuntimeError> {
        let value = self.resolve_value(expression)?;
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| RuntimeError::ValueTypeMismatch {
                expected: "string",
                actual: json_type_name(&value),
            })
    }

    /// 原子提交一次 Set Variables 节点已经全部求值成功的字段集合。
    pub(crate) fn commit_variables(&mut self, assignments: BTreeMap<String, Value>) {
        self.variables.extend(assignments);
    }

    /// 建立当前 input/vars/nodes 与可选原生 result 的一致求值快照。
    pub(crate) fn value_scope(
        &self,
        result: Option<&BTreeMap<String, Value>>,
    ) -> RuntimeValueScope {
        RuntimeValueScope::new(
            &self.workflow_inputs,
            &self.variables,
            &self.node_outputs,
            result,
        )
    }

    /// 在调用方提供的冻结快照上解析表达式，保证批量映射顺序无关。
    pub(crate) fn resolve_in_scope(
        &self,
        expression: &ValueExpr,
        scope: &RuntimeValueScope,
    ) -> Result<Value, RuntimeError> {
        match expression {
            ValueExpr::Literal { value } => Ok(value.clone()),
            ValueExpr::Ref { source, pointer } => {
                if !validate_json_pointer(pointer) {
                    return Err(RuntimeError::InvalidValuePointer {
                        pointer: pointer.clone(),
                    });
                }
                let root = match source {
                    ValueSource::WorkflowInput { key } => {
                        scope
                            .input
                            .get(key)
                            .ok_or_else(|| RuntimeError::ValueUnavailable {
                                description: format!("workflow input '{key}' is unavailable"),
                            })?
                    }
                    ValueSource::Variable { name } => {
                        scope
                            .variables
                            .get(name)
                            .ok_or_else(|| RuntimeError::ValueUnavailable {
                                description: format!("runtime variable '{name}' is unavailable"),
                            })?
                    }
                    ValueSource::Node { node_id } => {
                        scope
                            .nodes
                            .get(node_id)
                            .ok_or_else(|| RuntimeError::ValueUnavailable {
                                description: format!(
                                    "published outputs for node '{node_id}' are unavailable"
                                ),
                            })?
                    }
                };
                root.pointer(pointer)
                    .cloned()
                    .ok_or_else(|| RuntimeError::ValuePointerNotFound {
                        pointer: pointer.clone(),
                    })
            }
            ValueExpr::Expression { source } => {
                evaluate_expression(&self.value_plan, source, scope)
            }
        }
    }
}

/// 返回不泄漏具体业务值的 JSON 类型名称。
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use argusflow_core::{ValueExpr, ValueSource};
    use serde_json::{Value, json};
    use uuid::Uuid;

    use super::{NodeOutcome, RunContext};

    #[test]
    fn resolves_whole_nodes_and_json_pointer_fields() {
        let inputs = json!({ "order": { "id": "ACME-10086" } })
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
            NodeOutcome::values(BTreeMap::from([
                ("text".to_owned(), Value::String("ACME-10086".to_owned())),
                ("count".to_owned(), Value::from(1)),
            ])),
        );

        assert_eq!(
            context
                .resolve_value(&ValueExpr::node("read-order", ""))
                .expect("whole node output should resolve"),
            json!({ "count": 1, "text": "ACME-10086" }),
        );
        assert_eq!(
            context
                .resolve_text(&ValueExpr::Ref {
                    source: ValueSource::Node {
                        node_id: "read-order".to_owned(),
                    },
                    pointer: "/text".to_owned(),
                })
                .expect("node pointer should resolve"),
            "ACME-10086",
        );
    }

    #[test]
    fn retry_outcomes_replace_previous_published_outputs() {
        let mut context = RunContext::new(Uuid::new_v4(), Default::default(), Default::default());
        context.record_outcome(
            "worker".to_owned(),
            NodeOutcome::values(BTreeMap::from([("attempt".to_owned(), json!(1))])),
        );
        context.record_outcome(
            "worker".to_owned(),
            NodeOutcome::values(BTreeMap::from([("attempt".to_owned(), json!(2))])),
        );

        assert_eq!(
            context
                .resolve_value(&ValueExpr::node("worker", ""))
                .unwrap(),
            json!({ "attempt": 2 })
        );
    }

    #[test]
    fn text_consumers_report_json_type_mismatches() {
        let context = RunContext::new(Uuid::new_v4(), Default::default(), Default::default());

        assert!(matches!(
            context.resolve_text(&ValueExpr::Literal {
                value: json!({ "not": "text" }),
            }),
            Err(crate::RuntimeError::ValueTypeMismatch {
                expected: "string",
                ..
            })
        ));
    }
}
