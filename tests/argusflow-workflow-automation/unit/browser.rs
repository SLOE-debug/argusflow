use super::*;
use argusflow_browser::{Browser, BrowserConfig};
use argusflow_core::{Operation, OperationOptions};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn shared_operation_survives_successful_browser_calls() {
    let server = cdp::Server::start().await;
    let operation = Operation::new(OperationOptions::default());
    let browser =
        Browser::connect_with_operation(&server.endpoint, BrowserConfig::default(), &operation)
            .await
            .unwrap();
    assert!(!operation.is_cancelled());
    let pages = browser.pages_with_operation(&operation).await.unwrap();
    assert!(!operation.is_cancelled());
    let page = browser
        .attach_with_operation(pages[0].target_id(), &operation)
        .await
        .unwrap();
    page.navigate_with_operation("https://fixture.invalid/next", &operation)
        .await
        .unwrap();
    assert!(!operation.is_cancelled());
    page.detach_with_operation(&operation).await.unwrap();
    assert!(!operation.is_cancelled());
    browser.shutdown_with_operation(&operation).await.unwrap();
    assert!(
        !server
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call["method"] == "Browser.close")
    );
}

#[tokio::test]
async fn workflow_closes_owned_page_and_detaches_external_page() {
    let server = cdp::Server::start().await;
    let nodes = vec![
        task(
            "connect",
            "browser.connect",
            json!({}),
            vec![("endpoint", Expr::text(&server.endpoint))],
            vec![],
            vec![("browser", "browser")],
        ),
        task(
            "attach",
            "browser.attach",
            json!({}),
            vec![("target_id", Expr::text("external-page"))],
            vec![("browser", "browser")],
            vec![("page", "external")],
        ),
        task(
            "new",
            "browser.new_page",
            json!({}),
            vec![("url", Expr::text("https://fixture.invalid/"))],
            vec![("browser", "browser")],
            vec![("page", "own")],
        ),
        task(
            "navigate",
            "browser.navigate",
            json!({}),
            vec![("url", Expr::text("https://fixture.invalid/next"))],
            vec![("page", "own")],
            vec![],
        ),
    ];
    let plan = prepare(flow(nodes, vec![]), &registry()).unwrap();
    let engine = WorkflowEngine::new();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let result = handle.wait().await.unwrap();
    assert_eq!(result.status, RunStatus::Completed, "{:?}", result.error);
    assert_eq!(engine.retained_resources().await, 0);
    let calls = server.calls.lock().unwrap();
    assert!(
        calls
            .iter()
            .any(|call| call["method"] == "Target.closeTarget"
                && call["params"]["targetId"] == "own-page")
    );
    assert!(
        calls
            .iter()
            .any(|call| call["method"] == "Target.detachFromTarget"
                && call["params"]["sessionId"] == "session-external-page")
    );
    assert!(!calls.iter().any(|call| call["method"] == "Browser.close"
        || call["method"] == "Target.closeTarget"
            && call["params"]["targetId"] == "external-page"));
}

#[tokio::test]
async fn aql_query_adapter_returns_explicit_coordinate_snapshot() {
    let server = cdp::Server::start().await;
    let nodes = vec![
        task(
            "connect",
            "browser.connect",
            json!({}),
            vec![("endpoint", Expr::text(&server.endpoint))],
            vec![],
            vec![("browser", "browser")],
        ),
        task(
            "attach",
            "browser.attach",
            json!({}),
            vec![("target_id", Expr::text("external-page"))],
            vec![("browser", "browser")],
            vec![("page", "page")],
        ),
        task(
            "source",
            "source.dom",
            json!({}),
            vec![],
            vec![("page", "page")],
            vec![("source", "source")],
        ),
        task(
            "query",
            "aql.query",
            json!({"query":"button()"}),
            vec![],
            vec![("source", "source")],
            vec![],
        ),
    ];
    let first = Expr::Index {
        value: Box::new(Expr::NodeOutput {
            node: "query".into(),
            output: "matches".into(),
        }),
        index: Box::new(Expr::int(0)),
    };
    let outputs = vec![
        (
            "space",
            Expr::Field {
                value: Box::new(first.clone()),
                field: "space".into(),
            },
            ValueType::Text,
        ),
        (
            "bounds",
            Expr::Field {
                value: Box::new(first),
                field: "bounds".into(),
            },
            ValueType::List(Box::new(ValueType::Float)),
        ),
    ];
    let plan = prepare(flow(nodes, outputs), &registry()).unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), handle.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.status, RunStatus::Completed, "{:?}", result.error);
    assert_eq!(
        result.outputs["space"],
        Value::Text("frame_css_pixels".into())
    );
    assert_eq!(
        result.outputs["bounds"],
        Value::List([10., 20., 50., 50.].into_iter().map(Value::Float).collect())
    );
}

#[tokio::test]
async fn duplicate_page_claim_fails_before_second_attach() {
    let server = cdp::Server::start().await;
    let nodes = vec![
        task(
            "connect",
            "browser.connect",
            json!({}),
            vec![("endpoint", Expr::text(&server.endpoint))],
            vec![],
            vec![("browser", "b")],
        ),
        task(
            "one",
            "browser.attach",
            json!({}),
            vec![("target_id", Expr::text("external-page"))],
            vec![("browser", "b")],
            vec![("page", "one")],
        ),
        task(
            "two",
            "browser.attach",
            json!({}),
            vec![("target_id", Expr::text("external-page"))],
            vec![("browser", "b")],
            vec![("page", "two")],
        ),
    ];
    let plan = prepare(flow(nodes, vec![]), &registry()).unwrap();
    let mut handle = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Busy
    );
    assert_eq!(
        server
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call["method"] == "Target.attachToTarget")
            .count(),
        1
    );
}

#[test]
fn unknown_config_and_unsafe_retry_are_compile_errors() {
    let invalid = task(
        "invalid",
        "browser.connect",
        json!({"ignored":true}),
        vec![("endpoint", Expr::text("x"))],
        vec![],
        vec![("browser", "b")],
    );
    assert_eq!(
        prepare(flow(vec![invalid], vec![]), &registry())
            .err()
            .unwrap()[0]
            .code,
        DiagnosticCode::Task
    );
    let mut unsafe_retry = task(
        "create",
        "browser.connect",
        json!({}),
        vec![("endpoint", Expr::text("x"))],
        vec![],
        vec![("browser", "b")],
    );
    if let Action::Task { task } = &mut unsafe_retry.action {
        task.retry = Some(Retry {
            max_attempts: 2,
            initial_delay_ms: 1,
            max_delay_ms: 1,
            errors: vec![ErrorKind::Busy],
        });
    }
    assert_eq!(
        prepare(flow(vec![unsafe_retry], vec![]), &registry())
            .err()
            .unwrap()[0]
            .code,
        DiagnosticCode::Task
    );
}

#[test]
#[cfg(windows)]
fn lifecycle_json_prepares_without_starting_native_resources() {
    let flow = Workflow::from_json(include_str!("../fixtures/automation-lifecycle.json")).unwrap();
    assert!(prepare(flow, &registry()).is_ok());
}
