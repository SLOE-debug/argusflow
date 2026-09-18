use super::*;
use crate::runtime::messages::DesktopRunStatus;

#[tokio::test]
async fn separate_runs_preserve_all_events_and_terminal_errors() {
    let directory = tempfile::tempdir().unwrap();
    let mut first = RunLogFile::create(directory.path()).await.unwrap();
    let second = RunLogFile::create(directory.path()).await.unwrap();
    assert_ne!(first.path(), second.path());
    let mut snapshot = RunSnapshot {
        id: "1".into(),
        workflow: "workflow".into(),
        documents: vec![],
        status: DesktopRunStatus::Running,
        logs: vec![],
        omitted: 0,
        outputs: serde_json::json!({}),
        errors: vec![],
    };
    first.begin(&snapshot).await;
    for index in 0..5002 {
        first
            .append(&LogEntry {
                sequence: index.to_string(),
                elapsed_ms: index.to_string(),
                kind: "node_completed".into(),
                level: "info".into(),
                message: "完成\n下一行".into(),
                path: vec![],
            })
            .await;
    }
    let live = tokio::fs::read_to_string(first.path()).await.unwrap();
    assert_eq!(live.lines().count(), 5003);
    snapshot.status = DesktopRunStatus::Failed;
    snapshot.errors.push("input：能力调用失败".into());
    first.finish(&snapshot, 5010).await;
    assert!(first.failure().is_none());
    let text = tokio::fs::read_to_string(first.path()).await.unwrap();
    let records: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 5004);
    assert_eq!(records[1]["entry"]["elapsed_ms"], "0");
    assert_eq!(records.last().unwrap()["status"], "failed");
    assert_eq!(records.last().unwrap()["errors"][0], "input：能力调用失败");
    drop(first);
    drop(second);
}

#[tokio::test]
async fn unwritable_directory_is_reported_before_run() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("file");
    tokio::fs::write(&file, b"occupied").await.unwrap();
    assert!(RunLogFile::create(&file).await.is_err());
}
