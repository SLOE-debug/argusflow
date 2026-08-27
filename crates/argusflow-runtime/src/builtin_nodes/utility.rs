use std::{sync::Arc, time::Duration};

use argusflow_core::{ExecutionEventKind, ValueExpr, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeEvent, NodeExecution, NodeFlow, NodeValidationContext, PreparedNode, RunContext,
    RuntimeError, ValidationIssue, ValidationIssueCode, ValueInput,
    value_runtime::format_runtime_value,
};

/// Log 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LogPayload {
    /// 写入执行事件流的静态消息。
    message: String,
}

/// Debug 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DebugPayload {
    /// 执行时解析并展示的任意 JSON 表达式。
    value: ValueExpr,
}

/// Delay 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DelayPayload {
    /// 暂停时长，单位为毫秒。
    milliseconds: u64,
}

/// 创建冻结的 Log 节点。
pub(super) fn prepare_log(payload: LogPayload) -> Arc<dyn PreparedNode> {
    Arc::new(LogNode {
        message: payload.message,
    })
}

/// 创建冻结的 Debug 节点。
pub(super) fn prepare_debug(payload: DebugPayload) -> Arc<dyn PreparedNode> {
    Arc::new(DebugNode {
        value: payload.value,
    })
}

/// 创建冻结的 Delay 节点。
pub(super) fn prepare_delay(payload: DelayPayload) -> Arc<dyn PreparedNode> {
    Arc::new(DelayNode {
        milliseconds: payload.milliseconds,
    })
}

/// 写入静态日志事件的节点。
#[derive(Debug)]
struct LogNode {
    /// 已在 prepare 阶段冻结的消息。
    message: String,
}

/// 显式解析值表达式并写入调试日志的节点。
#[derive(Debug)]
struct DebugNode {
    /// 已解码的任意 JSON 值来源。
    value: ValueExpr,
}

/// 在确定时间边界内暂停当前路径的节点。
#[derive(Debug)]
struct DelayNode {
    /// 暂停毫秒数，执行前已经校验范围。
    milliseconds: u64,
}

#[async_trait]
impl PreparedNode for LogNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Log".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        if self.message.trim().is_empty() {
            vec![context.issue(
                ValidationIssueCode::EmptyLogMessage,
                "Log 节点的消息不能为空",
            )]
        } else {
            Vec::new()
        }
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        Ok(NodeExecution {
            events: vec![NodeEvent {
                kind: ExecutionEventKind::Log,
                message: Some(self.message.clone()),
                payload: None,
            }],
            ..NodeExecution::default()
        })
    }
}

#[async_trait]
impl PreparedNode for DebugNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Debug Output".to_owned()
    }

    fn value_inputs(&self) -> Vec<crate::ValueInput<'_>> {
        vec![ValueInput::json(&self.value)]
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let value = context.resolve_value(&self.value)?;
        Ok(NodeExecution {
            events: vec![NodeEvent {
                kind: ExecutionEventKind::Log,
                // Debug 节点由用户显式放置，因此允许把解析值写入开发日志。
                message: Some(format_runtime_value(&value)),
                payload: None,
            }],
            ..NodeExecution::default()
        })
    }
}

#[async_trait]
impl PreparedNode for DelayNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        format!("Delay {}ms", self.milliseconds)
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        if !(1..=60_000).contains(&self.milliseconds) {
            vec![context.issue(
                ValidationIssueCode::InvalidDelay,
                "Delay 节点必须在 1 到 60000 毫秒之间",
            )]
        } else {
            Vec::new()
        }
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        tokio::time::sleep(Duration::from_millis(self.milliseconds)).await;
        Ok(NodeExecution::default())
    }
}
