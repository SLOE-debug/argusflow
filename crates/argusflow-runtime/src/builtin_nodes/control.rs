use std::sync::Arc;

use argusflow_core::{ConditionOperator, ControlPortId, ValueExpr, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeExecution, NodeFlow, NodeValidationContext, PreparedNode, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode, ValueInput,
};

/// Start 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartPayload {}

/// End 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EndPayload {}

/// Condition 节点的强类型运行时值比较 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConditionPayload {
    /// 在当前 RunContext 上求值的左表达式。
    left: ValueExpr,
    /// 对 JSON 操作数执行的安全比较。
    operator: ConditionOperator,
    /// 二元运算符的右表达式；一元运算符必须为空。
    right: Option<ValueExpr>,
}

/// 创建冻结的 Start 节点。
pub(super) fn prepare_start(_payload: StartPayload) -> Arc<dyn PreparedNode> {
    Arc::new(StartNode)
}

/// 创建冻结的 End 节点。
pub(super) fn prepare_end(_payload: EndPayload) -> Arc<dyn PreparedNode> {
    Arc::new(EndNode)
}

/// 创建冻结的 Condition 节点。
pub(super) fn prepare_condition(payload: ConditionPayload) -> Arc<dyn PreparedNode> {
    Arc::new(ConditionNode {
        left: payload.left,
        operator: payload.operator,
        right: payload.right,
    })
}

/// 唯一工作流入口。
#[derive(Debug)]
struct StartNode;

/// 唯一工作流出口。
#[derive(Debug)]
struct EndNode;

/// 已解码的运行时二元条件分支节点。
#[derive(Debug)]
struct ConditionNode {
    /// 分支选择时读取的数据表达式。
    left: ValueExpr,
    /// 与核心层纯比较语义共享的运算符。
    operator: ConditionOperator,
    /// 可选右值表达式。
    right: Option<ValueExpr>,
}

#[async_trait]
impl PreparedNode for StartNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Start
    }

    fn label(&self) -> String {
        "Start".to_owned()
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        Ok(NodeExecution::default())
    }
}

#[async_trait]
impl PreparedNode for EndNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::End
    }

    fn label(&self) -> String {
        "End".to_owned()
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        Ok(NodeExecution::default())
    }
}

#[async_trait]
impl PreparedNode for ConditionNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Branch {
            ports: vec![ControlPortId::new("true"), ControlPortId::new("false")],
        }
    }

    fn label(&self) -> String {
        "Condition".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let valid_shape = self.operator.is_unary() == self.right.is_none();
        if valid_shape {
            Vec::new()
        } else {
            vec![context.issue(
                ValidationIssueCode::InvalidCondition,
                "一元条件不能携带右表达式，二元条件必须携带右表达式",
            )]
        }
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        std::iter::once(&self.left)
            .chain(self.right.iter())
            .map(ValueInput::json)
            .collect()
    }

    fn select_branch(&self, context: &RunContext) -> Result<Option<ControlPortId>, RuntimeError> {
        let left = context.resolve_optional_value(&self.left)?;
        let right = self
            .right
            .as_ref()
            .map(|expression| context.resolve_value(expression))
            .transpose()?;
        let matched = self
            .operator
            .evaluate(left.as_ref(), right.as_ref())
            .map_err(|error| RuntimeError::NodeExecution {
                message: error.to_string(),
            })?;
        Ok(Some(ControlPortId::new(if matched {
            "true"
        } else {
            "false"
        })))
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        Ok(NodeExecution::default())
    }
}
