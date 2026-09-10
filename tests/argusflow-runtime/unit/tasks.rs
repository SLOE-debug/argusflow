use super::{support::*, tasks::*};
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

#[tokio::test]
async fn failed_resource_delivery_contract_still_cleans_created_resource() {
    let probe = Arc::new(Probe::default());
    let mut acquire = task("acquire", "invalid");
    configure(&mut acquire)
        .resource_outputs
        .insert("out".into(), "resource".into());
    let plan = prepare(
        workflow(vec![scope("root", vec![acquire], vec![])], Fields::new()),
        &registry(vec![("invalid", Mode::InvalidAcquire)], &probe),
    )
    .unwrap();
    let engine = WorkflowEngine::new();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Contract
    );
    assert_eq!(probe.cleaned.load(Ordering::SeqCst), 1);
    assert_eq!(engine.retained_resources().await, 0);
}

#[test]
fn compiler_panic_returns_located_diagnostic() {
    let probe = Arc::new(Probe::default());
    let flow = workflow(
        vec![scope("root", vec![task("invalid", "panic")], vec![])],
        Fields::new(),
    );
    let error = prepare(flow, &registry(vec![("panic", Mode::CompilePanic)], &probe))
        .err()
        .unwrap()
        .remove(0);
    assert_eq!(error.code, DiagnosticCode::Task);
    assert_eq!(error.node.as_deref(), Some("invalid"));
}

#[tokio::test]
async fn safe_queries_retry_but_uncertain_effects_do_not() {
    for (mode, expected, status) in [
        (Mode::ReadRetry, 3, RunStatus::Completed),
        (Mode::EffectFailure, 1, RunStatus::Failed),
    ] {
        let probe = Arc::new(Probe::default());
        let mut action = task("action", "test");
        retry(&mut action);
        let plan = prepare(
            workflow(vec![scope("root", vec![action], vec![])], Fields::new()),
            &registry(vec![("test", mode)], &probe),
        )
        .unwrap();
        let mut handle = WorkflowEngine::new()
            .start(plan, RunInputs::default(), RunOptions::default())
            .unwrap();
        assert_eq!(handle.wait().await.unwrap().status, status);
        assert_eq!(probe.attempts.load(Ordering::SeqCst), expected);
    }
}

#[tokio::test]
async fn output_mapping_failure_never_reexecutes_successful_effect() {
    let probe = Arc::new(Probe::default());
    let mut action = task("action", "effect");
    action.output_bindings.insert(
        "bad".into(),
        Expr::binary(BinaryOp::Divide, Expr::int(1), Expr::int(0)),
    );
    let plan = prepare(
        workflow(vec![scope("root", vec![action], vec![])], Fields::new()),
        &registry(vec![("effect", Mode::EffectSuccess)], &probe),
    )
    .unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let result = handle.wait().await.unwrap();
    let error = result.error.as_ref().unwrap();
    assert_eq!(error.kind, ErrorKind::Expression);
    assert_eq!(error.effect, argusflow_core::Effect::Unconfirmed);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn resources_created_in_each_iteration_are_reclaimed() {
    let probe = Arc::new(Probe::default());
    let root = scope(
        "root",
        vec![node(
            "each",
            Action::ForEach {
                items: Expr::List {
                    item_type: ValueType::Int,
                    items: vec![Expr::int(1), Expr::int(2), Expr::int(3)],
                },
                item: "item".into(),
                index: "index".into(),
                body: "body".into(),
                max_iterations: None,
            },
        )],
        vec![],
    );
    let mut acquire = task("acquire", "acquire");
    configure(&mut acquire)
        .resource_outputs
        .insert("out".into(), "resource".into());
    let flow = workflow(
        vec![root, scope("body", vec![acquire], vec![])],
        Fields::new(),
    );
    let plan = prepare(flow, &registry(vec![("acquire", Mode::Acquire)], &probe)).unwrap();
    let engine = WorkflowEngine::with_limits(1, 1).unwrap();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Completed);
    assert_eq!(probe.cleaned.load(Ordering::SeqCst), 3);
    assert_eq!(engine.retained_resources().await, 0);
}

#[tokio::test]
async fn cleanup_failure_retains_quota_until_explicit_retry() {
    let probe = Arc::new(Probe::default());
    probe.reject_cleanup.store(true, Ordering::SeqCst);
    let mut acquire = task("acquire", "acquire");
    configure(&mut acquire)
        .resource_outputs
        .insert("out".into(), "resource".into());
    let plan = prepare(
        workflow(vec![scope("root", vec![acquire], vec![])], Fields::new()),
        &registry(vec![("acquire", Mode::Acquire)], &probe),
    )
    .unwrap();
    let engine = WorkflowEngine::with_limits(1, 1).unwrap();
    let mut handle = engine
        .start(plan.clone(), RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Failed);
    assert_eq!(engine.retained_resources().await, 1);
    let mut second = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(
        second.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Busy
    );
    probe.reject_cleanup.store(false, Ordering::SeqCst);
    engine.retry_cleanup(Duration::from_secs(1)).await.unwrap();
    assert_eq!(engine.retained_resources().await, 0);
}

#[tokio::test]
async fn dependent_resources_close_in_reverse_order() {
    let probe = Arc::new(Probe::default());
    let mut a = task("a", "acquire");
    configure(&mut a)
        .resource_outputs
        .insert("out".into(), "parent".into());
    let mut b = task("b", "child");
    configure(&mut b)
        .resources
        .insert("in".into(), "parent".into());
    configure(&mut b)
        .resource_outputs
        .insert("out".into(), "child".into());
    let plan = prepare(
        workflow(vec![scope("root", vec![a, b], vec![])], Fields::new()),
        &registry(
            vec![("acquire", Mode::Acquire), ("child", Mode::AcquireChild)],
            &probe,
        ),
    )
    .unwrap();
    let engine = WorkflowEngine::new();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Completed);
    assert_eq!(*probe.cleanup_order.lock().unwrap(), [2, 1]);
}

#[tokio::test]
async fn host_borrowed_resource_is_never_closed() {
    let probe = Arc::new(Probe::default());
    let mut use_resource = task("use", "use");
    configure(&mut use_resource)
        .resources
        .insert("in".into(), "host".into());
    let mut flow = workflow(
        vec![scope("root", vec![use_resource], vec![])],
        Fields::new(),
    );
    flow.resources.insert("host".into(), "test.resource".into());
    let plan = prepare(flow, &registry(vec![("use", Mode::Use)], &probe)).unwrap();
    let mut inputs = RunInputs::default();
    inputs.resources.insert(
        "host".into(),
        Arc::new(TestResource {
            probe: probe.clone(),
            id: 1,
        }),
    );
    let mut handle = WorkflowEngine::new()
        .start(plan, inputs, RunOptions::default())
        .unwrap();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Completed);
    assert_eq!(probe.cleaned.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancellation_runs_finally_and_cleans_acquired_resources() {
    let probe = Arc::new(Probe::default());
    let mut acquire = task("acquire", "acquire");
    configure(&mut acquire)
        .resource_outputs
        .insert("out".into(), "resource".into());
    let root = scope(
        "root",
        vec![
            acquire,
            node(
                "try",
                Action::Try {
                    body: "body".into(),
                    catches: vec![],
                    finally: Some("finally".into()),
                },
            ),
        ],
        vec![],
    );
    let body = scope("body", vec![task("hang", "hang")], vec![]);
    let finally = scope("finally", vec![task("finish", "effect")], vec![]);
    let plan = prepare(
        workflow(vec![root, body, finally], Fields::new()),
        &registry(
            vec![
                ("acquire", Mode::Acquire),
                ("hang", Mode::Hang),
                ("effect", Mode::EffectSuccess),
            ],
            &probe,
        ),
    )
    .unwrap();
    let engine = WorkflowEngine::new();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let mut events = handle.subscribe();
    loop {
        if let EventRead::Event(event) = events.recv().await
            && event.kind == EventKind::NodeStarted
            && event.path.last().unwrap().node.as_deref() == Some("hang")
        {
            break;
        }
    }
    handle.cancel();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Cancelled);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(probe.cleaned.load(Ordering::SeqCst), 1);
    assert_eq!(engine.retained_resources().await, 0);
}

#[tokio::test]
async fn extension_panic_still_runs_finally_and_terminates() {
    let probe = Arc::new(Probe::default());
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
    let plan = prepare(
        workflow(
            vec![
                root,
                scope("body", vec![task("panic", "panic")], vec![]),
                scope("finally", vec![task("finish", "effect")], vec![]),
            ],
            Fields::new(),
        ),
        &registry(
            vec![("panic", Mode::Panic), ("effect", Mode::EffectSuccess)],
            &probe,
        ),
    )
    .unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Contract
    );
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn slow_event_subscriber_gets_gap_and_final_result_remains_readable() {
    let root = scope(
        "root",
        vec![
            let_int("x", "x", Expr::int(0)),
            node(
                "loop",
                Action::While {
                    condition: less(Expr::var("x"), Expr::int(200)),
                    body: "body".into(),
                    max_iterations: None,
                },
            ),
        ],
        vec![],
    );
    let body = scope(
        "body",
        vec![assign("increment", "x", add(Expr::var("x"), Expr::int(1)))],
        vec![],
    );
    let plan = prepare(
        workflow(vec![root, body], Fields::new()),
        &NodeRegistry::new(),
    )
    .unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let mut events = handle.subscribe();
    assert_eq!(handle.wait().await.unwrap().status, RunStatus::Completed);
    assert!(matches!(events.recv().await, EventRead::Gap(n) if n > 0));
    assert_eq!(handle.result().unwrap().status, RunStatus::Completed);
}
