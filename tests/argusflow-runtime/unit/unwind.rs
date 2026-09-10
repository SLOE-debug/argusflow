use super::{support::*, tasks::*};
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

#[tokio::test]
async fn cancellation_during_cleanup_preserves_first_error_and_blocks_catch() {
    let probe = Arc::new(Probe::default());
    probe.hold_cleanup.store(true, Ordering::SeqCst);
    let mut acquire = task("acquire", "acquire");
    configure(&mut acquire)
        .resource_outputs
        .insert("out".into(), "owned".into());
    let flow = workflow(
        vec![
            scope(
                "root",
                vec![node(
                    "try",
                    Action::Try {
                        body: "body".into(),
                        catches: vec![Catch {
                            errors: vec![ErrorKind::User],
                            scope: "catch".into(),
                            error_name: "error".into(),
                        }],
                        finally: Some("finally".into()),
                    },
                )],
                vec![],
            ),
            scope(
                "body",
                vec![
                    acquire,
                    node(
                        "original",
                        Action::Fail {
                            code: "original".into(),
                        },
                    ),
                ],
                vec![],
            ),
            scope("catch", vec![task("must-not-catch", "effect")], vec![]),
            scope("finally", vec![task("finish", "effect")], vec![]),
        ],
        Fields::new(),
    );
    let plan = prepare(
        flow,
        &registry(
            vec![("acquire", Mode::Acquire), ("effect", Mode::EffectSuccess)],
            &probe,
        ),
    )
    .unwrap();
    let engine = WorkflowEngine::with_limits(1, 1).unwrap();
    let mut handle = engine
        .start(
            plan,
            RunInputs::default(),
            RunOptions {
                cleanup_timeout: Duration::from_millis(50),
                ..RunOptions::default()
            },
        )
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), probe.cleanup_started.notified())
        .await
        .unwrap();
    handle.cancel();
    let result = handle.wait().await.unwrap();
    let error = result.error.as_ref().unwrap();
    assert_eq!(error.message, "original");
    assert_eq!(error.path.last().unwrap().node.as_deref(), Some("original"));
    assert!(
        error
            .secondary
            .iter()
            .any(|error| error.kind == ErrorKind::Cancelled)
    );
    assert!(
        error
            .secondary
            .iter()
            .any(|error| error.kind == ErrorKind::Cleanup)
    );
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(engine.retained_resources().await, 1);
    probe.hold_cleanup.store(false, Ordering::SeqCst);
    engine.retry_cleanup(Duration::from_secs(1)).await.unwrap();
    assert_eq!(engine.retained_resources().await, 0);
}

#[tokio::test]
async fn exhausted_business_stack_still_runs_finally() {
    let probe = Arc::new(Probe::default());
    let flow = workflow(
        vec![
            scope(
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
            ),
            scope("body", vec![task("must-not-run", "effect")], vec![]),
            scope("finally", vec![task("finish", "effect")], vec![]),
        ],
        Fields::new(),
    );
    let plan = prepare(
        flow,
        &registry(vec![("effect", Mode::EffectSuccess)], &probe),
    )
    .unwrap();
    let mut handle = WorkflowEngine::new()
        .start(
            plan,
            RunInputs::default(),
            RunOptions {
                max_depth: 1,
                ..RunOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Limit
    );
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn finally_can_break_its_own_loop_without_overriding_return() {
    let flow = workflow(
        vec![
            scope(
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
            ),
            scope(
                "body",
                vec![node(
                    "return",
                    Action::Return {
                        values: [("answer".into(), Expr::int(7))].into(),
                    },
                )],
                vec![],
            ),
            scope(
                "finally",
                vec![node(
                    "loop",
                    Action::While {
                        condition: Expr::boolean(true),
                        body: "exit".into(),
                        max_iterations: Some(2),
                    },
                )],
                vec![],
            ),
            scope("exit", vec![node("break", Action::Break)], vec![]),
        ],
        int_fields(&["answer"]),
    );
    assert_output(&run(flow).await, "answer", 7);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_state_releases_run_permit_before_wait_returns() {
    let plan = prepare(
        workflow(vec![scope("root", vec![], vec![])], Fields::new()),
        &NodeRegistry::new(),
    )
    .unwrap();
    let engine = WorkflowEngine::with_limits(1, 1).unwrap();
    for _ in 0..100 {
        let mut handle = engine
            .start(plan.clone(), RunInputs::default(), RunOptions::default())
            .unwrap();
        assert_eq!(handle.wait().await.unwrap().status, RunStatus::Completed);
        engine.retry_cleanup(Duration::from_secs(1)).await.unwrap();
    }
}
