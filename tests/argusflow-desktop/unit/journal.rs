use super::*;
use argusflow_runtime::RunError;
use argusflow_workflow::{ErrorKind, Value, Values};
use std::sync::{Arc, Mutex as SyncMutex};
use tauri::ipc::InvokeResponseBody;

fn snapshot() -> RunSnapshot {
    RunSnapshot {
        id: "1".into(),
        workflow: "flow".into(),
        documents: vec!["flow".into()],
        status: DesktopRunStatus::Running,
        logs: Vec::new(),
        omitted: 0,
        outputs: serde_json::json!({}),
        errors: Vec::new(),
    }
}

fn entry(sequence: usize) -> LogEntry {
    LogEntry {
        sequence: sequence.to_string(),
        elapsed_ms: sequence.to_string(),
        kind: "node_completed".into(),
        level: "info".into(),
        message: "执行完成".into(),
        path: Vec::new(),
    }
}

fn channel(messages: &Arc<SyncMutex<Vec<serde_json::Value>>>) -> Channel<RunMessage> {
    let messages = messages.clone();
    Channel::new(move |body| {
        if let InvokeResponseBody::Json(source) = body {
            messages
                .lock()
                .unwrap()
                .push(serde_json::from_str(&source).unwrap());
        }
        Ok(())
    })
}

#[tokio::test]
async fn terminal_status_waits_for_complete_outputs_and_new_subscription_starts_with_snapshot() {
    let journal = RunJournal::default();
    journal.begin(snapshot(), Channel::new(|_| Ok(()))).await;
    journal.append(entry(1)).await;
    journal.progress(RunStatus::Completed).await;
    let snapshot = journal.snapshot().await.unwrap();
    assert_eq!(snapshot.status, DesktopRunStatus::Cleaning);
    let messages = Arc::new(SyncMutex::new(Vec::new()));
    journal.subscribe(channel(&messages)).await;
    journal.append(entry(2)).await;
    journal
        .finish(
            &RunResult {
                run_id: 1,
                status: RunStatus::Completed,
                outputs: Values::from([("answer".into(), Value::Int(9007199254740993))]),
                error: None,
            },
            3,
        )
        .await;
    let messages = messages.lock().unwrap();
    assert_eq!(messages[0]["type"], "snapshot");
    assert_eq!(messages[0]["snapshot"]["logs"][0]["sequence"], "1");
    assert_eq!(messages[1]["type"], "log");
    assert_eq!(messages[2]["snapshot"]["status"], "completed");
    assert_eq!(
        messages[2]["snapshot"]["outputs"]["answer"]["value"],
        "9007199254740993"
    );
}

#[tokio::test]
async fn final_error_and_event_gaps_obey_the_log_budget_and_survive_channel_failure() {
    let journal = RunJournal::default();
    // 模拟页面订阅已失效；日志和错误仍由宿主保存。
    journal
        .begin(
            snapshot(),
            Channel::new(|_| Err(std::io::Error::other("closed").into())),
        )
        .await;
    for sequence in 0..LOG_LIMIT {
        journal.append(entry(sequence)).await;
    }
    journal
        .append(LogEntry {
            kind: "gap".into(),
            level: "warning".into(),
            ..entry(LOG_LIMIT)
        })
        .await;
    let mut error = RunError::new(ErrorKind::User, "业务失败");
    error
        .secondary
        .push(RunError::new(ErrorKind::Cleanup, "资源清理失败"));
    journal
        .finish(
            &RunResult {
                run_id: 1,
                status: RunStatus::Failed,
                outputs: Values::new(),
                error: Some(error),
            },
            10,
        )
        .await;
    let result = journal.snapshot().await.unwrap();
    assert_eq!(result.status, DesktopRunStatus::Failed);
    assert_eq!(result.logs.len(), LOG_LIMIT);
    assert_eq!(result.omitted, 2);
    assert_eq!(result.logs.first().unwrap().sequence, "2");
    assert_eq!(result.logs[LOG_LIMIT - 2].kind, "gap");
    assert_eq!(result.logs.last().unwrap().kind, "error");
    assert_eq!(result.errors.len(), 2);
    let messages = Arc::new(SyncMutex::new(Vec::new()));
    journal.subscribe(channel(&messages)).await;
    assert_eq!(
        messages.lock().unwrap()[0]["snapshot"]["errors"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
