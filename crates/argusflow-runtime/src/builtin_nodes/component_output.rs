use std::{collections::BTreeMap, sync::Arc};

use argusflow_core::{ValueExpr, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeExecution, NodeFlow, NodeOutcome, PreparedNode, RunContext, RuntimeError, ValueInput,
    ValueTypeId,
};

/// 组件展开器生成的隐藏输出代理 payload。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ComponentOutputPayload {
    /// 组件公开输出名到内部表达式的冻结映射。
    outputs: BTreeMap<String, ValueExpr>,
}

/// 把输出代理 payload 冻结为线性运行时节点。
pub(super) fn prepare(payload: ComponentOutputPayload) -> Arc<dyn PreparedNode> {
    Arc::new(ComponentOutputNode {
        outputs: payload.outputs,
    })
}

/// 在内部出口前解析全部显式组件输出的隐藏节点。
#[derive(Debug)]
struct ComponentOutputNode {
    /// 展开后只引用已支配内部节点的输出表达式。
    outputs: BTreeMap<String, ValueExpr>,
}

#[async_trait]
impl PreparedNode for ComponentOutputNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Component Output".to_owned()
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        self.outputs.values().map(ValueInput::json).collect()
    }

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        self.outputs.contains_key(name).then(ValueTypeId::json)
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let outputs = self
            .outputs
            .iter()
            .map(|(name, expression)| {
                context
                    .resolve_value(expression)
                    .map(|value| (name.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(NodeExecution {
            outcome: NodeOutcome::values(outputs),
            events: Vec::new(),
            ..NodeExecution::default()
        })
    }
}
