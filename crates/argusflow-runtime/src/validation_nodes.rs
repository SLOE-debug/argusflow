use std::{collections::HashSet, path::Path};

use argusflow_core::{
    ApplicationSpec, BackendPreference, CommandOperation, CommandRunner, TargetLocator,
    TargetScope, WorkflowDefinition, WorkflowNode, WorkflowNodeKind,
};
use argusflow_query::parse_stored_query;

use crate::{ValidationIssue, ValidationIssueCode, validator::issue};

/// 校验一个节点独立于图结构的字段和权限约束。
pub(crate) fn validate_node_parameters(
    node: &WorkflowNode,
    workflow: &WorkflowDefinition,
    issues: &mut Vec<ValidationIssue>,
) {
    match &node.kind {
        WorkflowNodeKind::Start | WorkflowNodeKind::Debug { .. } | WorkflowNodeKind::End => {}
        WorkflowNodeKind::Log { message } => {
            if message.trim().is_empty() {
                issues.push(issue(
                    ValidationIssueCode::EmptyLogMessage,
                    "Log 节点的消息不能为空",
                    Some(node.id.clone()),
                    None,
                ));
            }
        }
        WorkflowNodeKind::Delay { milliseconds } => {
            if !(1..=60_000).contains(milliseconds) {
                issues.push(issue(
                    ValidationIssueCode::InvalidDelay,
                    "Delay 节点必须在 1 到 60000 毫秒之间",
                    Some(node.id.clone()),
                    None,
                ));
            }
        }
        WorkflowNodeKind::Condition { predicate } => {
            if let Err(error) = predicate.evaluate(&workflow.variables) {
                issues.push(issue(
                    ValidationIssueCode::InvalidCondition,
                    error.to_string(),
                    Some(node.id.clone()),
                    None,
                ));
            }
        }
        WorkflowNodeKind::Application { spec } => {
            validate_application_spec(spec, &node.id, issues);
        }
        WorkflowNodeKind::Ui { operation } => {
            let target = operation.target();
            if matches!(&target.scope, TargetScope::Application { .. })
                && matches!(target.backend_preference, BackendPreference::BrowserCdp)
            {
                issues.push(issue(
                    ValidationIssueCode::InvalidBackendPreference,
                    "应用资源当前不提供 Browser CDP 能力，不能强制使用 browser_cdp 后端",
                    Some(node.id.clone()),
                    None,
                ));
            }
            match &target.locator {
                TargetLocator::Query { query } => validate_aql_query(query, &node.id, issues),
                TargetLocator::Visual { query } => {
                    if query.text.trim().is_empty() {
                        issues.push(issue(
                            ValidationIssueCode::InvalidAqlQuery,
                            "视觉目标文字不能为空",
                            Some(node.id.clone()),
                            None,
                        ));
                    }
                }
                TargetLocator::Coordinate { .. } => {}
            }
        }
        WorkflowNodeKind::Command { operation } => {
            validate_command(operation, workflow, &node.id, issues);
        }
    }
}

/// 保留 parser 的精确 AQL 行列和修复建议。
fn validate_aql_query(
    query: &argusflow_core::AqlQuery,
    node_id: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let Err(error) = parse_stored_query(query) else {
        return;
    };
    let help = error
        .help
        .as_deref()
        .map(|help| format!("；建议：{help}"))
        .unwrap_or_default();
    issues.push(issue(
        ValidationIssueCode::InvalidAqlQuery,
        format!("AQL 查询无效：{error}{help}"),
        Some(node_id.to_owned()),
        None,
    ));
}

/// 在运行前拒绝无法形成确定应用身份和等待边界的配置。
fn validate_application_spec(
    application: &ApplicationSpec,
    node_id: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if !Path::new(application.executable_path.trim()).is_absolute() {
        issues.push(issue(
            ValidationIssueCode::InvalidApplicationSpec,
            "应用 EXE 必须使用绝对路径",
            Some(node_id.to_owned()),
            None,
        ));
    }
    if application.window_title.value().trim().is_empty() {
        issues.push(issue(
            ValidationIssueCode::InvalidApplicationSpec,
            "应用窗口标题匹配文本不能为空",
            Some(node_id.to_owned()),
            None,
        ));
    }
    if !(100..=60_000).contains(&application.launch_timeout_ms) {
        issues.push(issue(
            ValidationIssueCode::InvalidApplicationSpec,
            "应用启动超时必须在 100 到 60000 毫秒之间",
            Some(node_id.to_owned()),
            None,
        ));
    }
}

/// 校验 runner 判别字段、资源上限和显式工作流权限。
fn validate_command(
    command: &CommandOperation,
    workflow: &WorkflowDefinition,
    node_id: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let valid_shape = match command.runner {
        CommandRunner::Direct => command.program.is_some() && command.script.is_none(),
        CommandRunner::PowerShell | CommandRunner::Cmd => {
            command.program.is_none() && command.arguments.is_empty() && command.script.is_some()
        }
    };
    if !valid_shape {
        issues.push(issue(
            ValidationIssueCode::InvalidCommand,
            "命令字段与所选运行方式不匹配",
            Some(node_id.to_owned()),
            None,
        ));
    }
    if !(1..=3_600_000).contains(&command.timeout_ms) {
        issues.push(issue(
            ValidationIssueCode::InvalidCommand,
            "命令超时必须在 1 到 3600000 毫秒之间",
            Some(node_id.to_owned()),
            None,
        ));
    }
    if command.accepted_exit_codes.is_empty() {
        issues.push(issue(
            ValidationIssueCode::InvalidCommand,
            "命令至少需要一个可接受退出代码",
            Some(node_id.to_owned()),
            None,
        ));
    }
    const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;
    if !(1..=MAX_CAPTURE_BYTES).contains(&command.max_stdout_bytes)
        || !(1..=MAX_CAPTURE_BYTES).contains(&command.max_stderr_bytes)
    {
        issues.push(issue(
            ValidationIssueCode::InvalidCommand,
            "stdout/stderr 上限必须在 1 字节到 16 MiB 之间",
            Some(node_id.to_owned()),
            None,
        ));
    }
    validate_environment(command, node_id, issues);

    let permissions = workflow.permissions;
    let allowed = permissions.process_spawn
        && (!matches!(command.runner, CommandRunner::PowerShell) || permissions.powershell)
        && (!matches!(command.runner, CommandRunner::Cmd) || permissions.cmd);
    if !allowed {
        issues.push(issue(
            ValidationIssueCode::CommandPermissionDenied,
            "工作流权限未授权所选命令运行方式",
            Some(node_id.to_owned()),
            None,
        ));
    }
}

/// 环境变量名称必须唯一、非空且不能包含 Windows 赋值分隔符。
fn validate_environment(
    command: &CommandOperation,
    node_id: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut names = HashSet::new();
    for binding in &command.environment {
        let normalized = binding.name.to_uppercase();
        if binding.name.trim().is_empty() || binding.name.contains('=') || !names.insert(normalized)
        {
            issues.push(issue(
                ValidationIssueCode::InvalidCommand,
                "环境变量名称必须非空、不含等号且忽略大小写后唯一",
                Some(node_id.to_owned()),
                None,
            ));
        }
    }
}
