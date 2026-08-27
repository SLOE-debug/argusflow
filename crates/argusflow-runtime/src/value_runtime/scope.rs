use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};

use crate::NodeOutcome;

/// 一次值求值共享的不可变 JSON 快照。
#[derive(Debug, Clone)]
pub(crate) struct RuntimeValueScope {
    /// 本次运行启动时冻结的输入对象。
    pub(crate) input: Map<String, Value>,
    /// 当前 Runtime Variables 的一致快照。
    pub(crate) variables: Map<String, Value>,
    /// 已成功节点的 Published Outputs 一致快照。
    pub(crate) nodes: Map<String, Value>,
    /// 当前节点原生输出，仅输出映射阶段存在。
    pub(crate) result: Option<Map<String, Value>>,
}

impl RuntimeValueScope {
    /// 从 RunContext 数据面建立不会随本轮计算变化的深拷贝快照。
    pub(crate) fn new(
        input: &Map<String, Value>,
        variables: &Map<String, Value>,
        nodes: &HashMap<String, NodeOutcome>,
        result: Option<&BTreeMap<String, Value>>,
    ) -> Self {
        let nodes = nodes
            .iter()
            .map(|(node_id, outcome)| {
                let outputs = outcome
                    .outputs
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                (node_id.clone(), Value::Object(outputs))
            })
            .collect();
        let result = result.map(|outputs| {
            outputs
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        });
        Self {
            input: input.clone(),
            variables: variables.clone(),
            nodes,
            result,
        }
    }
}
