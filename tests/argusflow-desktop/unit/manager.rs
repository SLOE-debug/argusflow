use super::*;
use argusflow_workflow::{Action, Expr, Node, Scope, Workflow, WorkflowId};
use std::sync::Mutex as SyncMutex;
use tauri::ipc::InvokeResponseBody;

fn fixture(wait: i64) -> WorkflowBundle {
    WorkflowBundle {
        root: WorkflowId("flow".into()),
        workflows: BTreeMap::from([(
            WorkflowId("flow".into()),
            Workflow {
                name: "日志验收".into(),
                inputs: BTreeMap::new(),
                outputs: BTreeMap::new(),
                resources: BTreeMap::new(),
                root: "root".into(),
                subflows: BTreeMap::new(),
                scopes: vec![Scope::linear(
                    "root",
                    vec![Node::new(
                        "wait",
                        Action::Wait {
                            milliseconds: Expr::int(wait),
                        },
                    )],
                )],
            },
        )]),
    }
}
async fn manager() -> Arc<RunManager> {
    let manager = Arc::new(RunManager::default());
    let automation = Automation::from_host(Default::default()).await.unwrap();
    assert!(manager.automation.set(automation).is_ok());
    manager
}
async fn finished(manager: &RunManager) -> RunSnapshot {
    tokio::time::timeout(Duration::from_secs(2), async {
        while manager.active.lock().await.is_some() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    manager.snapshot().await.unwrap()
}
#[tokio::test]
async fn instant_execution_drains_events_before_final_snapshot_and_releases_handle() {
    let manager = manager().await;
    let messages = Arc::new(SyncMutex::new(Vec::new()));
    let received = messages.clone();
    let channel = Channel::new(move |body| {
        if let InvokeResponseBody::Json(source) = body {
            received
                .lock()
                .unwrap()
                .push(serde_json::from_str::<serde_json::Value>(&source).unwrap());
        }
        Ok(())
    });
    manager
        .start(fixture(0), Values::new(), channel)
        .await
        .unwrap();
    let snapshot = finished(&manager).await;
    assert_eq!(snapshot.status, DesktopRunStatus::Completed);
    assert_eq!(snapshot.logs.last().unwrap().kind, "run_finished");
    assert!(
        snapshot
            .logs
            .iter()
            .any(|entry| entry.kind == "node_completed")
    );
    {
        let messages = messages.lock().unwrap();
        let final_message = messages.last().unwrap();
        assert_eq!(final_message["type"], "snapshot");
        assert_eq!(final_message["snapshot"]["status"], "completed");
        assert_eq!(
            final_message["snapshot"]["logs"].as_array().unwrap().len(),
            snapshot.logs.len()
        );
    }
    manager
        .start(fixture(0), Values::new(), Channel::new(|_| Ok(())))
        .await
        .unwrap();
    assert_eq!(finished(&manager).await.status, DesktopRunStatus::Completed);
}
#[tokio::test]
async fn detached_frontend_can_query_final_cancelled_result_and_second_run_is_rejected() {
    let manager = manager().await;
    manager
        .start(fixture(10000), Values::new(), Channel::new(|_| Ok(())))
        .await
        .unwrap();
    assert!(
        manager
            .start(fixture(0), Values::new(), Channel::new(|_| Ok(())))
            .await
            .is_err()
    );
    manager
        .subscribe(Channel::new(
            |_| Err(std::io::Error::other("closed").into()),
        ))
        .await;
    manager.cancel().await.unwrap();
    assert_eq!(finished(&manager).await.status, DesktopRunStatus::Cancelled);
    manager.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn shutdown_wait_is_bounded_and_preserves_the_active_handle_for_retry() {
    let manager = manager().await;
    let (sender, mut receiver) = mpsc::channel(1);
    *manager.active.lock().await = Some(sender);
    let error = manager.shutdown().await.unwrap_err();
    assert!(error.contains("窗口已保留"));
    assert!(manager.active.lock().await.is_some());
    assert!(receiver.try_recv().is_ok());
    assert!(!manager.closing.load(Ordering::Acquire));
    *manager.active.lock().await = None;
    manager.shutdown().await.unwrap();
    assert!(
        manager
            .start(fixture(0), Values::new(), Channel::new(|_| Ok(())))
            .await
            .is_err()
    );
}
