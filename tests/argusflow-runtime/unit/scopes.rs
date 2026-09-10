use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;

#[tokio::test]
async fn lexical_assignment_and_shadowing_follow_let() {
    let root = scope(
        "root",
        vec![
            let_int("declare-x", "x", Expr::int(1)),
            node(
                "modify",
                Action::Block {
                    scope: "write".into(),
                },
            ),
            node(
                "shadow",
                Action::Block {
                    scope: "local".into(),
                },
            ),
        ],
        vec![("x", Expr::var("x")), ("local", output("shadow", "x"))],
    );
    let write = scope("write", vec![assign("write-x", "x", Expr::int(2))], vec![]);
    let local = scope(
        "local",
        vec![
            let_int("local-x", "x", Expr::int(9)),
            assign("local-change", "x", Expr::int(10)),
        ],
        vec![("x", Expr::var("x"))],
    );
    let result = run(workflow(
        vec![root, write, local],
        int_fields(&["x", "local"]),
    ))
    .await;
    assert_output(&result, "x", 2);
    assert_output(&result, "local", 10);
}

#[test]
fn inner_declaration_creates_temporal_dead_zone() {
    let root = scope(
        "root",
        vec![
            let_int("x", "x", Expr::int(1)),
            node(
                "block",
                Action::Block {
                    scope: "child".into(),
                },
            ),
        ],
        vec![],
    );
    let child = scope(
        "child",
        vec![
            assign("before", "x", Expr::int(2)),
            let_int("inner", "x", Expr::int(3)),
        ],
        vec![],
    );
    let errors = prepare(
        workflow(vec![root, child], Fields::new()),
        &NodeRegistry::new(),
    )
    .err()
    .unwrap();
    assert_eq!(errors[0].code, DiagnosticCode::Uninitialized);
}

#[test]
fn duplicate_declarations_and_implicit_conversions_are_rejected() {
    let duplicate = scope(
        "root",
        vec![
            let_int("a", "x", Expr::int(1)),
            let_int("b", "x", Expr::int(2)),
        ],
        vec![],
    );
    assert_eq!(
        prepare(
            workflow(vec![duplicate], Fields::new()),
            &NodeRegistry::new()
        )
        .err()
        .unwrap()[0]
            .code,
        DiagnosticCode::Reference
    );
    let wrong = scope("root", vec![let_int("a", "x", Expr::text("1"))], vec![]);
    assert_eq!(
        prepare(workflow(vec![wrong], Fields::new()), &NodeRegistry::new())
            .err()
            .unwrap()[0]
            .code,
        DiagnosticCode::Type
    );
}

#[tokio::test]
async fn assignment_is_atomic_when_second_expression_fails() {
    let root = scope(
        "root",
        vec![
            let_int("x", "x", Expr::int(1)),
            let_int("y", "y", Expr::int(2)),
            node(
                "try",
                Action::Try {
                    body: "body".into(),
                    catches: vec![Catch {
                        errors: vec![ErrorKind::Expression],
                        scope: "catch".into(),
                        error_name: "error".into(),
                    }],
                    finally: None,
                },
            ),
        ],
        vec![("x", Expr::var("x")), ("y", Expr::var("y"))],
    );
    let body = scope(
        "body",
        vec![node(
            "transaction",
            Action::Assign {
                assignments: vec![
                    Assignment {
                        name: "x".into(),
                        value: Expr::int(42),
                    },
                    Assignment {
                        name: "y".into(),
                        value: Expr::binary(BinaryOp::Divide, Expr::int(1), Expr::int(0)),
                    },
                ],
            },
        )],
        vec![],
    );
    let result = run(workflow(
        vec![root, body, scope("catch", vec![], vec![])],
        int_fields(&["x", "y"]),
    ))
    .await;
    assert_output(&result, "x", 1);
    assert_output(&result, "y", 2);
}

#[tokio::test]
async fn subflow_sees_globals_but_not_caller_locals_and_each_call_is_fresh() {
    let root = scope(
        "root",
        vec![
            let_int("global", "x", Expr::int(3)),
            node(
                "block",
                Action::Block {
                    scope: "caller".into(),
                },
            ),
        ],
        vec![("x", Expr::var("x")), ("answer", output("block", "answer"))],
    );
    let caller = scope(
        "caller",
        vec![
            let_int("shadow", "x", Expr::int(100)),
            call("call1", "add", vec![("amount", Expr::int(2))]),
            call("call2", "add", vec![("amount", Expr::int(4))]),
        ],
        vec![("answer", output("call2", "sum"))],
    );
    let sub = scope(
        "sub",
        vec![
            let_int(
                "scratch",
                "local",
                Expr::Input {
                    name: "amount".into(),
                },
            ),
            assign("global-add", "x", add(Expr::var("x"), Expr::var("local"))),
        ],
        vec![("sum", Expr::var("x"))],
    );
    let mut flow = workflow(vec![root, caller, sub], int_fields(&["x", "answer"]));
    flow.subflows.insert(
        "add".into(),
        Subflow {
            scope: "sub".into(),
            inputs: int_fields(&["amount"]),
            outputs: int_fields(&["sum"]),
            resources: Default::default(),
        },
    );
    let result = run(flow).await;
    assert_output(&result, "x", 9);
    assert_output(&result, "answer", 9);
}

#[test]
fn call_before_global_initialization_and_recursion_are_rejected() {
    let root = scope(
        "root",
        vec![
            call("early", "sub", vec![]),
            let_int("x", "x", Expr::int(1)),
        ],
        vec![],
    );
    let sub = scope("sub", vec![assign("write", "x", Expr::int(2))], vec![]);
    let mut flow = workflow(vec![root, sub], Fields::new());
    flow.subflows.insert(
        "sub".into(),
        Subflow {
            scope: "sub".into(),
            inputs: Fields::new(),
            outputs: Fields::new(),
            resources: Default::default(),
        },
    );
    assert_eq!(
        prepare(flow.clone(), &NodeRegistry::new()).err().unwrap()[0].code,
        DiagnosticCode::Uninitialized
    );
    flow.scopes[1] = scope("sub", vec![call("recursive", "sub", vec![])], vec![]);
    assert_eq!(
        prepare(flow, &NodeRegistry::new()).err().unwrap()[0].code,
        DiagnosticCode::Control
    );
}

#[test]
fn sibling_outputs_and_cross_scope_edges_are_rejected() {
    let root = scope(
        "root",
        vec![
            node(
                "block",
                Action::Block {
                    scope: "child".into(),
                },
            ),
            let_int("bad", "x", output("hidden", "value")),
        ],
        vec![],
    );
    let mut hidden = let_int("hidden", "local", Expr::int(1));
    hidden
        .output_bindings
        .insert("value".into(), Expr::var("local"));
    let child = scope("child", vec![hidden], vec![]);
    assert_eq!(
        prepare(
            workflow(vec![root, child], Fields::new()),
            &NodeRegistry::new()
        )
        .err()
        .unwrap()[0]
            .code,
        DiagnosticCode::Reference
    );
}
