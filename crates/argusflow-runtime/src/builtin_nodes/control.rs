use std::sync::Arc;

use argusflow_core::{ConditionPredicate, ControlPortId, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeExecution, NodeFlow, NodeValidationContext, PreparedNode, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode,
};

/// Start 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartPayload {}

/// End 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EndPayload {}

/// Condition 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConditionPayload {
    /// 在只读工作流变量上执行的安全条件。
    predicate: ConditionPredicate,
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
        predicate: payload.predicate,
    })
}

/// 唯一工作流入口。
#[derive(Debug)]
struct StartNode;

/// 唯一工作流出口。
#[derive(Debug)]
struct EndNode;

/// 已解码的二元条件分支节点。
#[derive(Debug)]
struct ConditionNode {
    /// 执行与校验共享的结构化谓词。
    predicate: ConditionPredicate,
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
        self.predicate
            .evaluate(&context.workflow.variables)
            .err()
            .map(|error| context.issue(ValidationIssueCode::InvalidCondition, error.to_string()))
            .into_iter()
            .collect()
    }

    fn select_branch(
        &self,
        variables: &serde_json::Value,
    ) -> Result<Option<ControlPortId>, RuntimeError> {
        let matched = self
            .predicate
            .evaluate(variables)
            .map_err(|error| RuntimeError::ExecutionInvariant(error.to_string()))?;
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
