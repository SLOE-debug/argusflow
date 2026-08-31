use std::sync::Arc;

use std::time::Duration;

use argusflow_core::{
    ConditionOperator, ControlPortId, ExecutionEventKind, ExecutionEventPayload, ValueExpr,
    WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeEvent, NodeExecution, NodeFlow, NodeValidationContext, PreparedNode, RunContext,
    RuntimeError, ValidationIssue, ValidationIssueCode, ValueInput, WorkflowTermination,
};

/// Start 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StartPayload {}

/// End 节点的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EndPayload {}

/// 有界循环门的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopPayload {
    /// 最多开始的循环体轮次数。
    max_iterations: u32,
    /// 从首次进入 Gate 起计算的总毫秒预算。
    timeout_ms: u64,
    /// 第二轮起每次进入循环体前的等待毫秒数。
    interval_ms: u64,
}

/// 显式失败终点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FailPayload {
    /// 工作流作者声明的稳定错误码。
    code: String,
    /// 在 Fail 执行时解析的用户可读消息。
    message: ValueExpr,
}

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

/// 创建冻结的 Loop Gate。
pub(super) fn prepare_loop(payload: LoopPayload) -> Arc<dyn PreparedNode> {
    Arc::new(LoopNode {
        max_iterations: payload.max_iterations,
        timeout_ms: payload.timeout_ms,
        interval_ms: payload.interval_ms,
    })
}

/// 创建冻结的显式失败终点。
pub(super) fn prepare_fail(payload: FailPayload) -> Arc<dyn PreparedNode> {
    Arc::new(FailNode {
        code: payload.code,
        message: payload.message,
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

/// 只允许由 Validator 建立单层结构化回边的有界循环门。
#[derive(Debug)]
struct LoopNode {
    /// 最大循环体轮次数。
    max_iterations: u32,
    /// 循环总时长预算。
    timeout_ms: u64,
    /// 相邻轮次之间的等待时间。
    interval_ms: u64,
}

/// 以预期业务错误结束流程的终点。
#[derive(Debug)]
struct FailNode {
    /// 稳定失败码。
    code: String,
    /// 运行时失败消息表达式。
    message: ValueExpr,
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

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
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
        Ok(NodeExecution {
            branch: Some(ControlPortId::new(if matched { "true" } else { "false" })),
            ..NodeExecution::default()
        })
    }
}

#[async_trait]
impl PreparedNode for LoopNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Loop {
            ports: vec![
                ControlPortId::new("iterate"),
                ControlPortId::new("exhausted"),
            ],
        }
    }

    fn label(&self) -> String {
        "重复执行".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let valid = (1..=10_000).contains(&self.max_iterations)
            && (1..=600_000).contains(&self.timeout_ms)
            && self.interval_ms <= 60_000;
        if valid {
            Vec::new()
        } else {
            vec![context.issue(
                ValidationIssueCode::InvalidLoop,
                "最多重复次数应为 1 到 10000，最长运行时间应为 1 到 600000 毫秒，每次间隔应为 0 到 60000 毫秒",
            )]
        }
    }

    async fn execute(
        &self,
        node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        if context.loop_iterations(node_id) > 0 && self.interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.interval_ms)).await;
        }
        match context.begin_loop_iteration(
            node_id,
            self.max_iterations,
            Duration::from_millis(self.timeout_ms),
        ) {
            Ok(iteration) => Ok(NodeExecution {
                branch: Some(ControlPortId::new("iterate")),
                events: vec![NodeEvent {
                    kind: ExecutionEventKind::LoopIteration,
                    message: Some(format!("开始第 {iteration} 次重复")),
                    payload: Some(ExecutionEventPayload::LoopIteration {
                        iteration,
                        max_iterations: self.max_iterations,
                    }),
                }],
                ..NodeExecution::default()
            }),
            Err(iterations) => Ok(NodeExecution {
                branch: Some(ControlPortId::new("exhausted")),
                events: vec![NodeEvent {
                    kind: ExecutionEventKind::LoopExhausted,
                    message: Some("已达到设置的重复次数或时间上限".to_owned()),
                    payload: Some(ExecutionEventPayload::LoopExhausted { iterations }),
                }],
                ..NodeExecution::default()
            }),
        }
    }
}

#[async_trait]
impl PreparedNode for FailNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::End
    }

    fn label(&self) -> String {
        "Fail".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        if self.code.trim().is_empty() {
            vec![context.issue(
                ValidationIssueCode::InvalidFailure,
                "请填写错误标识，方便在日志中找到这次问题",
            )]
        } else {
            Vec::new()
        }
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        vec![ValueInput::text(&self.message)]
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let message = context.resolve_text(&self.message)?;
        Ok(NodeExecution {
            events: vec![NodeEvent {
                kind: ExecutionEventKind::WorkflowFailureDeclared,
                message: Some(message.clone()),
                payload: Some(ExecutionEventPayload::WorkflowFailureDeclared {
                    code: self.code.clone(),
                }),
            }],
            termination: Some(WorkflowTermination {
                code: self.code.clone(),
                message,
            }),
            ..NodeExecution::default()
        })
    }
}
