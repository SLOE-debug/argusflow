use std::{collections::HashSet, sync::Arc};

use argusflow_core::{CommandOperation, CommandRunner, WorkflowPermissions};
use async_trait::async_trait;
use serde::Deserialize;

use super::typed_compiler;
use crate::{
    AccessSet, CommandExecutor, NodeCompiler, NodeEvent, NodeExecution, NodeFlow,
    NodeValidationContext, PreparedNode, ResourceAccessKey, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode, ValueInput, ValueTypeId,
};
use argusflow_core::{ExecutionEventKind, ExecutionEventPayload};

/// Command 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CommandPayload {
    /// 命令运行方式、输入输出边界和超时。
    operation: CommandOperation,
}

/// 创建无平台依赖的 Command 节点编译器。
pub(super) fn compiler() -> Arc<dyn NodeCompiler> {
    typed_compiler::<CommandPayload>("argus.command", prepare)
}

/// 将已解码 payload 冻结为 Command 节点。
fn prepare(payload: CommandPayload) -> Arc<dyn PreparedNode> {
    Arc::new(CommandNode {
        operation: payload.operation,
        executor: CommandExecutor,
    })
}

/// 已解码的命令节点与专用执行器。
#[derive(Debug)]
struct CommandNode {
    /// 执行与引用校验共享的命令契约。
    operation: CommandOperation,
    /// 独立于 UI Planner 的命令执行边界。
    executor: CommandExecutor,
}

#[async_trait]
impl PreparedNode for CommandNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        format!("Command {:?}", self.operation.runner)
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let valid_shape = match self.operation.runner {
            CommandRunner::Direct => {
                self.operation.program.is_some() && self.operation.script.is_none()
            }
            CommandRunner::PowerShell | CommandRunner::Cmd => {
                self.operation.program.is_none()
                    && self.operation.arguments.is_empty()
                    && self.operation.script.is_some()
            }
        };
        if !valid_shape {
            issues.push(context.issue(
                ValidationIssueCode::InvalidCommand,
                "命令字段与所选运行方式不匹配",
            ));
        }
        if !(1..=3_600_000).contains(&self.operation.timeout_ms) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidCommand,
                "命令超时必须在 1 到 3600000 毫秒之间",
            ));
        }
        if self.operation.accepted_exit_codes.is_empty() {
            issues.push(context.issue(
                ValidationIssueCode::InvalidCommand,
                "命令至少需要一个可接受退出代码",
            ));
        }
        const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;
        if !(1..=MAX_CAPTURE_BYTES).contains(&self.operation.max_stdout_bytes)
            || !(1..=MAX_CAPTURE_BYTES).contains(&self.operation.max_stderr_bytes)
        {
            issues.push(context.issue(
                ValidationIssueCode::InvalidCommand,
                "stdout/stderr 上限必须在 1 字节到 16 MiB 之间",
            ));
        }
        let mut names = HashSet::new();
        for binding in &self.operation.environment {
            let normalized = binding.name.to_uppercase();
            if binding.name.trim().is_empty()
                || binding.name.contains('=')
                || !names.insert(normalized)
            {
                issues.push(context.issue(
                    ValidationIssueCode::InvalidCommand,
                    "环境变量名称必须非空、不含等号且忽略大小写后唯一",
                ));
            }
        }
        if !context
            .workflow
            .permissions
            .allows_command(self.operation.runner)
        {
            issues.push(context.issue(
                ValidationIssueCode::CommandPermissionDenied,
                "工作流权限未授权所选命令运行方式",
            ));
        }
        issues
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        self.operation
            .program
            .iter()
            .chain(self.operation.arguments.iter())
            .chain(self.operation.script.iter())
            .chain(self.operation.working_directory.iter())
            .chain(
                self.operation
                    .environment
                    .iter()
                    .map(|binding| &binding.value),
            )
            .chain(self.operation.stdin.iter())
            .map(ValueInput::text)
            .collect()
    }

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        match name {
            "stdout" | "stderr" => Some(ValueTypeId::text()),
            "exit_code" => Some(ValueTypeId::json()),
            _ => None,
        }
    }

    fn access_set(&self, _node_id: &str, _context: &RunContext) -> Result<AccessSet, RuntimeError> {
        // 命令可能操作任意宿主状态；在引入更细粒度 capability key 前保持保守独占。
        Ok(AccessSet::exclusive(ResourceAccessKey::global(
            "system.command",
        )))
    }

    async fn execute(
        &self,
        _node_id: &str,
        permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let outcome = self
            .executor
            .execute(&self.operation, permissions, context)
            .await?;
        let exit_code = outcome
            .outputs
            .get("exit_code")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| {
                RuntimeError::ExecutionInvariant(
                    "command outcome did not contain an i32 exit_code".to_owned(),
                )
            })?;
        Ok(NodeExecution {
            outcome,
            events: vec![NodeEvent {
                kind: ExecutionEventKind::CommandExited,
                message: Some(format!("命令执行完成，退出代码 {exit_code}")),
                payload: Some(ExecutionEventPayload::CommandExited { exit_code }),
            }],
        })
    }
}
