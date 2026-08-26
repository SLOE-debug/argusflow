//! Command 节点完整生命周期 deadline 与 Windows 进程树边界测试。

use std::{
    io::{Write, stdout},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use argusflow_core::{
    CommandOperation, CommandRunner, EnvironmentBinding, ValueExpr, WorkflowPermissions,
};
use argusflow_runtime::{CommandError, CommandExecutor, RunContext, RuntimeError};
use serde_json::Map;
use uuid::Uuid;

const HELPER_MODE: &str = "ARGUSFLOW_COMMAND_HELPER_MODE";
const PARENT_MODE: &str = "parent";
const CHILD_MODE: &str = "child";
const OUTPUT_MODE: &str = "output";

/// 根 helper 退出后，继承输出管道的后台后代必须被 job 回收，不能让 drain 无限等待。
#[tokio::test]
async fn command_finishes_after_killing_a_descendant_that_inherits_output_pipes() {
    let operation = helper_operation("command_helper_parent", PARENT_MODE, 5_000, 64 * 1024);
    let permissions = WorkflowPermissions::direct_command_only();
    let context = RunContext::new(Uuid::new_v4(), Map::new(), Map::new());

    let outcome = tokio::time::timeout(
        Duration::from_secs(8),
        CommandExecutor.execute(&operation, &permissions, &context),
    )
    .await
    .expect("CommandExecutor must enforce its own complete lifecycle deadline")
    .expect("the root command should finish successfully");

    assert_eq!(
        outcome
            .outputs
            .get("exit_code")
            .and_then(|value| value.as_i64()),
        Some(0)
    );
    assert!(
        outcome
            .outputs
            .get("stdout")
            .and_then(|value| value.as_str())
            .is_some_and(|stdout| stdout.contains("parent-complete"))
    );
}

/// 输出一旦超过上限，Command 必须终止进程树并在自身 deadline 前返回上限错误。
#[tokio::test]
async fn command_terminates_immediately_when_stdout_exceeds_limit() {
    let operation = helper_operation("command_helper_output", OUTPUT_MODE, 10_000, 1_024);
    let context = RunContext::new(Uuid::new_v4(), Map::new(), Map::new());
    let started_at = Instant::now();

    let error = tokio::time::timeout(
        Duration::from_secs(5),
        CommandExecutor.execute(
            &operation,
            &WorkflowPermissions::direct_command_only(),
            &context,
        ),
    )
    .await
    .expect("output limit must stop execution before the node deadline")
    .expect_err("oversized stdout must fail the command");

    assert!(matches!(
        error,
        RuntimeError::Command(CommandError::OutputLimitExceeded {
            stream: "stdout",
            limit: 1_024,
        })
    ));
    assert!(started_at.elapsed() < Duration::from_secs(5));
}

/// 被测根进程：启动继承 stdout/stderr 的确定性后代后立即退出。
#[test]
fn command_helper_parent() {
    if std::env::var(HELPER_MODE).ok().as_deref() != Some(PARENT_MODE) {
        return;
    }
    let executable = std::env::current_exe().expect("test executable should be available");
    Command::new(executable)
        .args(["--exact", "command_helper_child", "--nocapture"])
        .env(HELPER_MODE, CHILD_MODE)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("descendant helper should start");
    println!("parent-complete");
}

/// 被测后代进程：保持继承的输出管道打开，直到 Job Object 将其终止。
#[test]
fn command_helper_child() {
    if std::env::var(HELPER_MODE).ok().as_deref() == Some(CHILD_MODE) {
        thread::sleep(Duration::from_secs(30));
    }
}

/// 被测输出进程：持续写入大块数据，正常情况下不会自行退出。
#[test]
fn command_helper_output() {
    if std::env::var(HELPER_MODE).ok().as_deref() != Some(OUTPUT_MODE) {
        return;
    }
    let chunk = [b'x'; 8 * 1024];
    loop {
        if stdout().write_all(&chunk).is_err() {
            return;
        }
    }
}

/// 构造直接运行当前测试二进制中指定 helper 测试的命令。
fn helper_operation(
    test_name: &str,
    mode: &str,
    timeout_ms: u64,
    max_stdout_bytes: usize,
) -> CommandOperation {
    let executable = std::env::current_exe().expect("test executable should be available");
    CommandOperation {
        runner: CommandRunner::Direct,
        program: Some(ValueExpr::text(executable.to_string_lossy())),
        arguments: vec![
            ValueExpr::text("--exact"),
            ValueExpr::text(test_name),
            ValueExpr::text("--nocapture"),
        ],
        script: None,
        working_directory: None,
        environment: vec![EnvironmentBinding {
            name: HELPER_MODE.to_owned(),
            value: ValueExpr::text(mode),
        }],
        stdin: None,
        timeout_ms,
        accepted_exit_codes: vec![0],
        max_stdout_bytes,
        max_stderr_bytes: 64 * 1024,
    }
}
