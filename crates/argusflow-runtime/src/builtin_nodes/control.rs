use std::sync::Arc;

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
    /// 该容器拥有的 While 子作用域。
    body_scope_id: String,
    /// 最多开始的循环体轮次数。
    max_iterations: u32,
    /// 从首次进入 Gate 起计算的总毫秒预算。
    timeout_ms: u64,
    /// 第二轮起每次进入循环体前的等待毫秒数。
    interval_ms: u64,
}

/// While 子作用域入口的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopEntryPayload {}

/// While 子作用域继续出口的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopContinuePayload {}

/// While 子作用域完成出口的空 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LoopCompletePayload {}

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
        body_scope_id: payload.body_scope_id,
        max_iterations: payload.max_iterations,
        timeout_ms: payload.timeout_ms,
        interval_ms: payload.interval_ms,
    })
}

/// 创建 While 子作用域入口。
pub(super) fn prepare_loop_entry(_payload: LoopEntryPayload) -> Arc<dyn PreparedNode> {
    Arc::new(LoopEntryNode)
}

/// 创建 While 子作用域继续出口。
pub(super) fn prepare_loop_continue(_payload: LoopContinuePayload) -> Arc<dyn PreparedNode> {
    Arc::new(LoopContinueNode)
}

/// 创建 While 子作用域完成出口。
pub(super) fn prepare_loop_complete(_payload: LoopCompletePayload) -> Arc<dyn PreparedNode> {
    Arc::new(LoopCompleteNode)
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
    /// 唯一拥有的子作用域 ID。
    body_scope_id: String,
    /// 最大循环体轮次数。
    max_iterations: u32,
    /// 循环总时长预算。
    timeout_ms: u64,
    /// 相邻轮次之间的等待时间。
    interval_ms: u64,
}

/// While 子作用域固定入口。
#[derive(Debug)]
struct LoopEntryNode;

/// While 子作用域继续下一轮的固定出口。
#[derive(Debug)]
struct LoopContinueNode;

/// While 子作用域正常完成的固定出口。
#[derive(Debug)]
struct LoopCompleteNode;

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
                ControlPortId::new("completed"),
                ControlPortId::new("exhausted"),
            ],
            body_scope_id: self.body_scope_id.clone(),
            max_iterations: self.max_iterations,
            timeout_ms: self.timeout_ms,
            interval_ms: self.interval_ms,
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
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        // While 的进入、轮次和退出由显式执行栈统一编排；节点对象只冻结强类型配置。
        Ok(NodeExecution::default())
    }
}

macro_rules! impl_loop_boundary {
    ($node:ty, $flow:expr, $label:literal) => {
        #[async_trait]
        impl PreparedNode for $node {
            fn flow(&self) -> NodeFlow {
                $flow
            }
            fn label(&self) -> String {
                $label.to_owned()
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
    };
}

impl_loop_boundary!(LoopEntryNode, NodeFlow::LoopEntry, "循环开始");
impl_loop_boundary!(LoopContinueNode, NodeFlow::LoopContinue, "继续下一轮");
impl_loop_boundary!(LoopCompleteNode, NodeFlow::LoopComplete, "完成循环");

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
