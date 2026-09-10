use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;

#[tokio::test]
async fn nested_loops_reset_locals_and_accumulate_outer_variables() {
    let root = scope(
        "root",
        vec![
            let_int("total", "total", Expr::int(0)),
            let_int("outer", "outer", Expr::int(0)),
            node(
                "while",
                Action::While {
                    condition: less(Expr::var("outer"), Expr::int(3)),
                    body: "outer-body".into(),
                    max_iterations: None,
                },
            ),
        ],
        vec![("total", Expr::var("total"))],
    );
    let outer = scope(
        "outer-body",
        vec![
            let_int("inner", "inner", Expr::int(0)),
            node(
                "inner-while",
                Action::While {
                    condition: less(Expr::var("inner"), Expr::int(2)),
                    body: "inner-body".into(),
                    max_iterations: None,
                },
            ),
            assign("inc-outer", "outer", add(Expr::var("outer"), Expr::int(1))),
        ],
        vec![],
    );
    let inner = scope(
        "inner-body",
        vec![
            assign("inc-total", "total", add(Expr::var("total"), Expr::int(1))),
            assign("inc-inner", "inner", add(Expr::var("inner"), Expr::int(1))),
        ],
        vec![],
    );
    assert_output(
        &run(workflow(vec![root, outer, inner], int_fields(&["total"]))).await,
        "total",
        6,
    );
}

#[tokio::test]
async fn foreach_freezes_values_and_continue_runs_finally() {
    let root = scope(
        "root",
        vec![
            let_int("total", "total", Expr::int(0)),
            node(
                "each",
                Action::ForEach {
                    items: Expr::List {
                        item_type: ValueType::Int,
                        items: vec![Expr::int(2), Expr::int(3)],
                    },
                    item: "item".into(),
                    index: "index".into(),
                    body: "body".into(),
                    max_iterations: None,
                },
            ),
        ],
        vec![("total", Expr::var("total"))],
    );
    let body = scope(
        "body",
        vec![node(
            "try",
            Action::Try {
                body: "next".into(),
                catches: vec![],
                finally: Some("finally".into()),
            },
        )],
        vec![],
    );
    let next = scope("next", vec![node("continue", Action::Continue)], vec![]);
    let finally = scope(
        "finally",
        vec![assign(
            "accumulate",
            "total",
            add(Expr::var("total"), Expr::var("item")),
        )],
        vec![],
    );
    assert_output(
        &run(workflow(
            vec![root, body, next, finally],
            int_fields(&["total"]),
        ))
        .await,
        "total",
        5,
    );
}

#[tokio::test]
async fn return_passes_through_finally_and_preserves_result_snapshot() {
    let root = scope(
        "root",
        vec![
            let_int("x", "x", Expr::int(1)),
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
    let body = scope(
        "body",
        vec![node(
            "return",
            Action::Return {
                values: [("x".into(), Expr::var("x"))].into(),
            },
        )],
        vec![],
    );
    let finally = scope("finally", vec![assign("change", "x", Expr::int(2))], vec![]);
    assert_output(
        &run(workflow(vec![root, body, finally], int_fields(&["x"]))).await,
        "x",
        1,
    );
}

#[tokio::test]
async fn zero_iteration_loop_does_not_initialize_body() {
    let root = scope(
        "root",
        vec![node(
            "loop",
            Action::While {
                condition: Expr::boolean(false),
                body: "body".into(),
                max_iterations: None,
            },
        )],
        vec![("value", Expr::int(0))],
    );
    let body = scope(
        "body",
        vec![node(
            "fail",
            Action::Fail {
                code: "must-not-run".into(),
            },
        )],
        vec![],
    );
    assert_output(
        &run(workflow(vec![root, body], int_fields(&["value"]))).await,
        "value",
        0,
    );
}

#[tokio::test]
async fn budget_is_fatal_and_cannot_be_caught() {
    let root = scope(
        "root",
        vec![node(
            "loop",
            Action::While {
                condition: Expr::boolean(true),
                body: "empty".into(),
                max_iterations: Some(3),
            },
        )],
        vec![],
    );
    let result = run(workflow(
        vec![root, scope("empty", vec![], vec![])],
        Fields::new(),
    ))
    .await;
    assert_eq!(result.error.as_ref().unwrap().kind, ErrorKind::Limit);
}

#[tokio::test]
async fn branches_publish_only_declared_container_outputs() {
    let root = scope(
        "root",
        vec![node(
            "if",
            Action::If {
                condition: Expr::boolean(true),
                then_scope: "yes".into(),
                else_scope: "no".into(),
            },
        )],
        vec![("answer", output("if", "answer"))],
    );
    assert_output(
        &run(workflow(
            vec![
                root,
                scope("yes", vec![], vec![("answer", Expr::int(42))]),
                scope("no", vec![], vec![("answer", Expr::int(0))]),
            ],
            int_fields(&["answer"]),
        ))
        .await,
        "answer",
        42,
    );
}

#[test]
fn mismatched_branch_outputs_fail_preparation() {
    let root = scope(
        "root",
        vec![node(
            "if",
            Action::If {
                condition: Expr::boolean(true),
                then_scope: "yes".into(),
                else_scope: "no".into(),
            },
        )],
        vec![],
    );
    let flow = workflow(
        vec![
            root,
            scope("yes", vec![], vec![("v", Expr::int(1))]),
            scope("no", vec![], vec![("v", Expr::text("x"))]),
        ],
        Fields::new(),
    );
    assert_eq!(
        prepare(flow, &NodeRegistry::new()).err().unwrap()[0].code,
        DiagnosticCode::Type
    );
}
