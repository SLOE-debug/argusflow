//! Command 节点完整生命周期 deadline 与 Windows 进程树边界测试。

use std::time::Duration;

use argusflow_core::{CommandOperation, CommandRunner, ValueExpr, WorkflowPermissions};
use argusflow_runtime::{CommandExecutor, RunContext};
use serde_json::Map;
use uuid::Uuid;

/// 根 shell 退出后，继承输出管道的后台后代必须被 job 回收，不能让 drain 无限等待。
#[tokio::test]
async fn command_finishes_after_killing_a_descendant_that_inherits_output_pipes() {
    let operation = CommandOperation {
        runner: CommandRunner::Cmd,
        program: None,
        arguments: Vec::new(),
        script: Some(ValueExpr::text(
            r#"start "" /B cmd.exe /D /C "ping.exe 127.0.0.1 -n 30" & echo parent-complete"#,
        )),
        working_directory: None,
        environment: Vec::new(),
        stdin: None,
        timeout_ms: 3_000,
        accepted_exit_codes: vec![0],
        max_stdout_bytes: 64 * 1024,
        max_stderr_bytes: 64 * 1024,
    };
    let permissions = WorkflowPermissions {
        process_spawn: true,
        powershell: false,
        cmd: true,
    };
    let context = RunContext::new(Uuid::new_v4(), Map::new());

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        CommandExecutor.execute(&operation, permissions, &context),
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
