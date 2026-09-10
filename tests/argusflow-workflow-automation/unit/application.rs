use super::*;
use argusflow_core::{Operation, OperationOptions};
use argusflow_windows::{Application, ApplicationOptions};
use serde_json::json;
#[path = "../support/process_observation.rs"]
mod observation;

fn fixture() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/process_fixture.exe")
}
#[tokio::test]
#[ignore = "requires cargo build -p argusflow-workflow-automation --example process_fixture; launches only a hidden acceptance process"]
async fn managed_application_tree_exits_before_cleanup_succeeds() {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut external = observation::ExternalFixture::start(&fixture());
    let mut options = ApplicationOptions::new(fixture());
    options.visible = false;
    options.arguments = vec![
        "--report".into(),
        listener.local_addr().unwrap().to_string(),
    ];
    let operation = Operation::new(OperationOptions::default());
    let application = Application::launch(options, &operation).unwrap();
    let pid = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.read_u32_le().await.unwrap()
    })
    .await
    .unwrap();
    let descendant = observation::ObservedProcess::open(pid);
    assert!(descendant.is_running());
    assert!(!application.is_closed());
    application.shutdown(&operation).await.unwrap();
    assert!(application.is_closed());
    assert!(!descendant.is_running());
    assert!(external.is_running());
    application.shutdown(&operation).await.unwrap();
}
#[tokio::test]
#[ignore = "requires process_fixture; workflow launches only a hidden acceptance process"]
async fn workflow_failure_cleans_owned_application() {
    let executable = fixture().to_str().unwrap().to_owned();
    let nodes = vec![
        task(
            "launch",
            "application.launch",
            json!({"visible":false}),
            vec![
                ("executable", Expr::text(executable)),
                (
                    "arguments",
                    Expr::List {
                        item_type: ValueType::Text,
                        items: vec![],
                    },
                ),
            ],
            vec![],
            vec![("application", "app")],
        ),
        Node::new(
            "fail",
            Action::Fail {
                code: "expected".into(),
            },
        ),
    ];
    let plan = prepare(flow(nodes, vec![]), &registry()).unwrap();
    let engine = WorkflowEngine::new();
    let mut handle = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap();
    let result = handle.wait().await.unwrap();
    assert_eq!(result.error.as_ref().unwrap().message, "expected");
    assert!(result.error.as_ref().unwrap().secondary.is_empty());
    assert_eq!(engine.retained_resources().await, 0);
}
