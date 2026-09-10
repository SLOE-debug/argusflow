use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::time::Duration;

#[tokio::test]
async fn first_failure_keeps_finally_failure_as_secondary() {
    let root = scope(
        "root",
        vec![node(
            "try",
            Action::Try {
                body: "body".into(),
                catches: vec![],
                finally: Some("finally".into()),
            },
        )],
        vec![],
    );
    let body = scope(
        "body",
        vec![node(
            "first",
            Action::Fail {
                code: "primary".into(),
            },
        )],
        vec![],
    );
    let finally = scope(
        "finally",
        vec![node(
            "second",
            Action::Fail {
                code: "secondary".into(),
            },
        )],
        vec![],
    );
    let result = run(workflow(vec![root, body, finally], Fields::new())).await;
    let error = result.error.as_ref().unwrap();
    assert_eq!(error.message, "primary");
    assert_eq!(error.secondary[0].message, "secondary");
    assert_eq!(error.path.last().unwrap().node.as_deref(), Some("first"));
}

#[tokio::test]
async fn cancellation_is_terminal_and_events_close() {
    let root = scope(
        "root",
        vec![node(
            "wait",
            Action::Wait {
                milliseconds: Expr::int(60_000),
            },
        )],
        vec![],
    );
    let plan = prepare(workflow(vec![root], Fields::new()), &NodeRegistry::new()).unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let mut events = handle.subscribe();
    handle.cancel();
    let result = tokio::time::timeout(Duration::from_secs(1), handle.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.status, RunStatus::Cancelled);
    loop {
        if matches!(
            tokio::time::timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap(),
            EventRead::Closed
        ) {
            break;
        }
    }
}

#[tokio::test]
async fn node_timeout_can_be_caught_by_outer_try() {
    let root = scope(
        "root",
        vec![node(
            "try",
            Action::Try {
                body: "body".into(),
                catches: vec![Catch {
                    errors: vec![ErrorKind::Timeout],
                    scope: "catch".into(),
                    error_name: "error".into(),
                }],
                finally: None,
            },
        )],
        vec![],
    );
    let mut wait = node(
        "wait",
        Action::Wait {
            milliseconds: Expr::int(10_000),
        },
    );
    wait.timeout_ms = Some(10);
    let flow = workflow(
        vec![
            root,
            scope("body", vec![wait], vec![]),
            scope("catch", vec![], vec![]),
        ],
        Fields::new(),
    );
    let result = run(flow).await;
    assert_eq!(result.status, RunStatus::Completed, "{:?}", result.error);
}

#[tokio::test]
async fn same_prepared_plan_has_isolated_run_variables() {
    let root = scope(
        "root",
        vec![
            let_int("x", "x", Expr::int(0)),
            assign("increment", "x", add(Expr::var("x"), Expr::int(1))),
        ],
        vec![("x", Expr::var("x"))],
    );
    let plan = prepare(
        workflow(vec![root], int_fields(&["x"])),
        &NodeRegistry::new(),
    )
    .unwrap();
    let engine = WorkflowEngine::new();
    let mut a = engine
        .start(plan.clone(), RunInputs::default(), RunOptions::default())
        .unwrap();
    let mut b = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_output(&a.wait().await.unwrap(), "x", 1);
    assert_output(&b.wait().await.unwrap(), "x", 1);
}
