use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use argusflow_core::{CommandOperation, CommandRunner, WorkflowPermissions};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use crate::{NodeOutcome, RunContext, RuntimeError, command_job::CommandJob};

/// Command 节点准备或执行阶段的稳定错误边界。
#[derive(Debug, Error)]
pub enum CommandError {
    /// 工作流没有授权所选命令能力。
    #[error("command permission was denied: {message}")]
    PermissionDenied {
        /// 缺失的授权说明。
        message: String,
    },
    /// 命令参数组合不符合所选运行器契约。
    #[error("invalid command operation: {message}")]
    InvalidOperation {
        /// 无效参数说明。
        message: String,
    },
    /// 子进程无法启动或观察。
    #[error("command process failed: {message}")]
    ProcessFailed {
        /// I/O 或系统错误摘要。
        message: String,
    },
    /// 命令超过节点定义的执行时限。
    #[error("command timed out after {timeout_ms} ms")]
    Timeout {
        /// 配置的完整执行时限。
        timeout_ms: u64,
    },
    /// 子进程输出超过显式资源上限。
    #[error("command {stream} exceeded {limit} bytes")]
    OutputLimitExceeded {
        /// stdout 或 stderr。
        stream: &'static str,
        /// 配置的字节上限。
        limit: usize,
    },
    /// 进程以未被节点接受的代码退出。
    #[error("command exited with unaccepted code {exit_code}")]
    ExitCodeRejected {
        /// 实际退出代码。
        exit_code: i32,
    },
}

/// 准备并执行 CommandOperation 的独立节点执行器。
#[derive(Debug, Default)]
pub struct CommandExecutor;

impl CommandExecutor {
    /// 解析 ValueExpr、验证权限并执行命令，返回标准输出端口。
    pub async fn execute(
        &self,
        operation: &CommandOperation,
        permissions: WorkflowPermissions,
        context: &RunContext,
    ) -> Result<NodeOutcome, RuntimeError> {
        ensure_permissions(operation.runner, permissions)?;
        let mut command = prepare_command(operation, context)?;
        let stdin = operation
            .stdin
            .as_ref()
            .map(|expression| context.resolve_text(expression))
            .transpose()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(operation.timeout_ms);
        let job =
            CommandJob::create().map_err(|message| CommandError::ProcessFailed { message })?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        CommandJob::configure_command(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| CommandError::ProcessFailed {
                message: error.to_string(),
            })?;
        if let Err(message) = job.assign_and_resume(&child) {
            terminate_command(&job, &mut child);
            return Err(CommandError::ProcessFailed { message }.into());
        }
        let mut child_stdin = child.stdin.take();
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                terminate_command(&job, &mut child);
                return Err(CommandError::ProcessFailed {
                    message: "child stdout pipe was not created".to_owned(),
                }
                .into());
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                terminate_command(&job, &mut child);
                return Err(CommandError::ProcessFailed {
                    message: "child stderr pipe was not created".to_owned(),
                }
                .into());
            }
        };
        let stdout_limit = operation.max_stdout_bytes;
        let stderr_limit = operation.max_stderr_bytes;
        let mut stdout_task = tokio::spawn(read_bounded(stdout, stdout_limit));
        let mut stderr_task = tokio::spawn(read_bounded(stderr, stderr_limit));

        if let Some(stdin) = stdin {
            let mut stdin_pipe = match child_stdin.take() {
                Some(stdin_pipe) => stdin_pipe,
                None => {
                    terminate_command(&job, &mut child);
                    stdout_task.abort();
                    stderr_task.abort();
                    return Err(CommandError::ProcessFailed {
                        message: "child stdin pipe was not created".to_owned(),
                    }
                    .into());
                }
            };
            match tokio::time::timeout_at(deadline, stdin_pipe.write_all(stdin.as_bytes())).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    drop(stdin_pipe);
                    terminate_command(&job, &mut child);
                    stdout_task.abort();
                    stderr_task.abort();
                    return Err(CommandError::ProcessFailed {
                        message: error.to_string(),
                    }
                    .into());
                }
                Err(_) => {
                    drop(stdin_pipe);
                    terminate_command(&job, &mut child);
                    stdout_task.abort();
                    stderr_task.abort();
                    return Err(CommandError::Timeout {
                        timeout_ms: operation.timeout_ms,
                    }
                    .into());
                }
            }
        }
        // 显式关闭 stdin，避免等待输入结束的命令悬挂。
        drop(child_stdin);

        let lifecycle = async {
            let status = child
                .wait()
                .await
                .map_err(|error| CommandError::ProcessFailed {
                    message: error.to_string(),
                })?;
            // 根进程退出即终止仍存活的后代，确保继承的 stdout/stderr 写端可以关闭。
            job.terminate();
            let stdout = join_output(&mut stdout_task, "stdout", stdout_limit).await?;
            let stderr = join_output(&mut stderr_task, "stderr", stderr_limit).await?;
            Ok::<_, RuntimeError>((status, stdout, stderr))
        };
        let (status, stdout, stderr) = match tokio::time::timeout_at(deadline, lifecycle).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                terminate_command(&job, &mut child);
                stdout_task.abort();
                stderr_task.abort();
                return Err(error);
            }
            Err(_) => {
                terminate_command(&job, &mut child);
                stdout_task.abort();
                stderr_task.abort();
                return Err(CommandError::Timeout {
                    timeout_ms: operation.timeout_ms,
                }
                .into());
            }
        };
        let exit_code = status.code().ok_or_else(|| CommandError::ProcessFailed {
            message: "command terminated without an exit code".to_owned(),
        })?;
        if !operation.accepted_exit_codes.contains(&exit_code) {
            return Err(CommandError::ExitCodeRejected { exit_code }.into());
        }

        let mut outputs = BTreeMap::new();
        outputs.insert("exit_code".to_owned(), Value::from(exit_code));
        outputs.insert(
            "stdout".to_owned(),
            Value::String(String::from_utf8_lossy(&stdout).into_owned()),
        );
        outputs.insert(
            "stderr".to_owned(),
            Value::String(String::from_utf8_lossy(&stderr).into_owned()),
        );
        Ok(NodeOutcome::values(outputs))
    }
}

/// 同步触发整个 job 与根进程终止，不在已经耗尽的节点 deadline 之后继续等待。
fn terminate_command(job: &CommandJob, child: &mut tokio::process::Child) {
    job.terminate();
    let _ = child.start_kill();
}

/// 检查 WorkflowPermissions 是否覆盖命令运行器要求。
fn ensure_permissions(
    runner: CommandRunner,
    permissions: WorkflowPermissions,
) -> Result<(), CommandError> {
    if !permissions.process_spawn {
        return Err(CommandError::PermissionDenied {
            message: "process_spawn permission is required".to_owned(),
        });
    }
    if matches!(runner, CommandRunner::PowerShell) && !permissions.powershell {
        return Err(CommandError::PermissionDenied {
            message: "powershell permission is required".to_owned(),
        });
    }
    if matches!(runner, CommandRunner::Cmd) && !permissions.cmd {
        return Err(CommandError::PermissionDenied {
            message: "cmd permission is required".to_owned(),
        });
    }
    Ok(())
}

/// 把已解析参数冻结到 tokio Command，不拼接 shell 字符串。
fn prepare_command(
    operation: &CommandOperation,
    context: &RunContext,
) -> Result<Command, RuntimeError> {
    let mut command = match operation.runner {
        CommandRunner::Direct => {
            let program =
                operation
                    .program
                    .as_ref()
                    .ok_or_else(|| CommandError::InvalidOperation {
                        message: "Direct runner requires program".to_owned(),
                    })?;
            let program = context.resolve_text(program)?;
            if program.trim().is_empty() {
                return Err(CommandError::InvalidOperation {
                    message: "Direct program cannot be empty".to_owned(),
                }
                .into());
            }
            let mut command = Command::new(program);
            for argument in &operation.arguments {
                command.arg(context.resolve_text(argument)?);
            }
            command
        }
        CommandRunner::PowerShell => {
            let script = required_script(operation, context)?;
            let mut command = Command::new(system_executable(
                r"System32\WindowsPowerShell\v1.0\powershell.exe",
            )?);
            command.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ]);
            command
        }
        CommandRunner::Cmd => {
            let script = required_script(operation, context)?;
            let mut command = Command::new(system_executable(r"System32\cmd.exe")?);
            command.args(["/D", "/S", "/C", &script]);
            command
        }
    };
    if let Some(directory) = &operation.working_directory {
        let directory = context.resolve_text(directory)?;
        if !Path::new(&directory).is_dir() {
            return Err(CommandError::InvalidOperation {
                message: format!("working directory does not exist: {directory}"),
            }
            .into());
        }
        command.current_dir(directory);
    }
    for binding in &operation.environment {
        command.env(&binding.name, context.resolve_text(&binding.value)?);
    }
    Ok(command)
}

/// 从宿主 SystemRoot 构造固定系统工具路径，避免 shell runner 经过 PATH 搜索。
fn system_executable(relative_path: &str) -> Result<PathBuf, CommandError> {
    let system_root = std::env::var_os("SystemRoot")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandError::InvalidOperation {
            message: "SystemRoot is unavailable".to_owned(),
        })?;
    let executable = PathBuf::from(system_root).join(relative_path);
    if !executable.is_file() {
        return Err(CommandError::InvalidOperation {
            message: format!(
                "system command runner does not exist: {}",
                executable.display()
            ),
        });
    }
    Ok(executable)
}

/// 解析 PowerShell/CMD 必需的非空脚本。
fn required_script(
    operation: &CommandOperation,
    context: &RunContext,
) -> Result<String, RuntimeError> {
    let script = operation
        .script
        .as_ref()
        .ok_or_else(|| CommandError::InvalidOperation {
            message: "shell runner requires script".to_owned(),
        })?;
    let script = context.resolve_text(script)?;
    if script.trim().is_empty() {
        return Err(CommandError::InvalidOperation {
            message: "script cannot be empty".to_owned(),
        }
        .into());
    }
    Ok(script)
}

/// 持续排空子进程流并只保留限制内数据，避免管道阻塞或无界内存增长。
async fn read_bounded(
    mut reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<(Vec<u8>, bool), std::io::Error> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    Ok((retained, exceeded))
}

/// 解包输出读取任务并将 I/O、Join 和上限错误映射到命令边界。
async fn join_output(
    task: &mut tokio::task::JoinHandle<Result<(Vec<u8>, bool), std::io::Error>>,
    stream: &'static str,
    limit: usize,
) -> Result<Vec<u8>, RuntimeError> {
    let (output, exceeded) = task
        .await
        .map_err(|error| CommandError::ProcessFailed {
            message: error.to_string(),
        })?
        .map_err(|error| CommandError::ProcessFailed {
            message: error.to_string(),
        })?;
    if exceeded {
        return Err(CommandError::OutputLimitExceeded { stream, limit }.into());
    }
    Ok(output)
}
