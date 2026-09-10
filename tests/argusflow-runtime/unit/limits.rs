use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::time::Duration;

#[tokio::test]
async fn root_timeout_cannot_be_caught_as_local_timeout() {
    let flow = workflow(
        vec![
            scope(
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
            ),
            scope(
                "body",
                vec![node(
                    "wait",
                    Action::Wait {
                        milliseconds: Expr::int(60_000),
                    },
                )],
                vec![],
            ),
            scope("catch", vec![], vec![]),
        ],
        Fields::new(),
    );
    let plan = prepare(flow, &NodeRegistry::new()).unwrap();
    let mut handle = WorkflowEngine::new()
        .start(
            plan,
            RunInputs::default(),
            RunOptions {
                timeout: Duration::from_millis(20),
                ..RunOptions::default()
            },
        )
        .unwrap();
    let result = handle.wait().await.unwrap();
    assert_eq!(result.status, RunStatus::TimedOut);
    assert_eq!(result.error.as_ref().unwrap().kind, ErrorKind::RunTimeout);
}

#[tokio::test]
async fn nested_node_deadline_is_bounded_by_ancestor() {
    let mut parent = node(
        "parent",
        Action::Block {
            scope: "body".into(),
        },
    );
    parent.timeout_ms = Some(20);
    let mut wait = node(
        "child",
        Action::Wait {
            milliseconds: Expr::int(60_000),
        },
    );
    wait.timeout_ms = Some(60_000);
    let flow = workflow(
        vec![
            scope("root", vec![parent], vec![]),
            scope("body", vec![wait], vec![]),
        ],
        Fields::new(),
    );
    let result = tokio::time::timeout(Duration::from_secs(1), run(flow))
        .await
        .unwrap();
    assert_eq!(result.status, RunStatus::TimedOut);
    let error = result.error.as_ref().unwrap();
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert_eq!(
        error
            .path
            .iter()
            .map(|p| p.node.as_deref())
            .collect::<Vec<_>>(),
        [Some("parent"), Some("child")]
    );
}

#[tokio::test]
async fn assignment_expressions_share_one_budget() {
    let flow = workflow(
        vec![scope(
            "root",
            vec![
                let_int("a", "a", Expr::int(1)),
                let_int("b", "b", Expr::int(2)),
                node(
                    "assign",
                    Action::Assign {
                        assignments: vec![
                            Assignment {
                                name: "a".into(),
                                value: Expr::int(3),
                            },
                            Assignment {
                                name: "b".into(),
                                value: Expr::int(4),
                            },
                        ],
                    },
                ),
            ],
            vec![],
        )],
        Fields::new(),
    );
    let plan = prepare(flow, &NodeRegistry::new()).unwrap();
    let mut handle = WorkflowEngine::new()
        .start(
            plan,
            RunInputs::default(),
            RunOptions {
                expression_steps: 1,
                ..RunOptions::default()
            },
        )
        .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().error.as_ref().unwrap().kind,
        ErrorKind::Limit
    );
}

#[test]
fn empty_typed_collections_cannot_hide_invalid_type_structure() {
    let invalid = ValueType::Record([("".into(), ValueType::Int)].into());
    let flow = workflow(
        vec![scope(
            "root",
            vec![node(
                "wait",
                Action::Wait {
                    milliseconds: Expr::Function {
                        function: Function::Length,
                        arguments: vec![Expr::List {
                            item_type: invalid,
                            items: vec![],
                        }],
                    },
                },
            )],
            vec![],
        )],
        Fields::new(),
    );
    let diagnostic = prepare(flow, &NodeRegistry::new()).err().unwrap().remove(0);
    assert_eq!(diagnostic.code, DiagnosticCode::Type);
    assert_eq!(diagnostic.node.as_deref(), Some("wait"));
}

#[tokio::test]
async fn switch_break_targets_nearest_loop_and_rebuilds_outputs() {
    let mut emit = let_int("local", "local", Expr::var("item"));
    emit.output_bindings
        .insert("value".into(), Expr::var("local"));
    let flow = workflow(
        vec![
            scope(
                "root",
                vec![
                    let_int("sum", "sum", Expr::int(0)),
                    node(
                        "each",
                        Action::ForEach {
                            items: Expr::List {
                                item_type: ValueType::Int,
                                items: vec![Expr::int(2), Expr::int(3)],
                            },
                            item: "item".into(),
                            index: "index".into(),
                            body: "outer".into(),
                            max_iterations: None,
                        },
                    ),
                ],
                vec![("sum", Expr::var("sum"))],
            ),
            scope(
                "outer",
                vec![
                    emit,
                    node(
                        "inner",
                        Action::While {
                            condition: Expr::boolean(true),
                            body: "switch".into(),
                            max_iterations: Some(2),
                        },
                    ),
                    assign(
                        "accumulate",
                        "sum",
                        add(Expr::var("sum"), output("local", "value")),
                    ),
                ],
                vec![],
            ),
            scope(
                "switch",
                vec![node(
                    "select",
                    Action::Switch {
                        selector: Expr::var("item"),
                        cases: vec![SwitchCase {
                            value: Value::Int(2),
                            scope: "two".into(),
                        }],
                        default_scope: "default".into(),
                    },
                )],
                vec![],
            ),
            scope("two", vec![node("break-two", Action::Break)], vec![]),
            scope(
                "default",
                vec![node("break-default", Action::Break)],
                vec![],
            ),
        ],
        int_fields(&["sum"]),
    );
    assert_output(&run(flow).await, "sum", 5);
}
