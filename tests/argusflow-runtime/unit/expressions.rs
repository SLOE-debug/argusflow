use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;

#[tokio::test]
async fn checked_in_json_examples_execute_with_declared_results() {
    let loops = Workflow::from_json(include_str!("../fixtures/nested-loops.json")).unwrap();
    assert_output(&run(loops).await, "total", 6);
    let calls = Workflow::from_json(include_str!("../fixtures/subflow.json")).unwrap();
    let result = run(calls).await;
    assert_output(&result, "first", 2);
    assert_output(&result, "second", 7);
}

#[tokio::test]
async fn short_circuit_skips_division_by_zero() {
    let bad = Expr::binary(
        BinaryOp::Equal,
        Expr::binary(BinaryOp::Divide, Expr::int(1), Expr::int(0)),
        Expr::int(0),
    );
    let expression = Expr::binary(BinaryOp::And, Expr::boolean(false), bad);
    let flow = workflow(
        vec![scope("root", vec![], vec![("value", expression)])],
        [("value".into(), ValueType::Bool)].into(),
    );
    let result = run(flow).await;
    assert_eq!(result.status, RunStatus::Completed);
    assert_eq!(result.outputs["value"], Value::Bool(false));
}

#[tokio::test]
async fn records_lists_and_optional_functions_preserve_types() {
    let record = Expr::Record {
        fields: [(
            "v".into(),
            Expr::List {
                item_type: ValueType::Int,
                items: vec![Expr::int(4)],
            },
        )]
        .into(),
    };
    let index = Expr::Index {
        value: Box::new(Expr::Field {
            value: Box::new(record),
            field: "v".into(),
        }),
        index: Box::new(Expr::int(0)),
    };
    let value = Expr::Function {
        function: Function::OrElse,
        arguments: vec![
            Expr::Some {
                value: Box::new(index),
            },
            Expr::binary(BinaryOp::Divide, Expr::int(1), Expr::int(0)),
        ],
    };
    assert_output(
        &run(workflow(
            vec![scope("root", vec![], vec![("v", value)])],
            int_fields(&["v"]),
        ))
        .await,
        "v",
        4,
    );
}

#[tokio::test]
async fn overflow_is_reported_as_expression_failure() {
    let flow = workflow(
        vec![scope(
            "root",
            vec![],
            vec![("v", add(Expr::int(i64::MAX), Expr::int(1)))],
        )],
        int_fields(&["v"]),
    );
    assert_eq!(
        run(flow).await.error.as_ref().unwrap().kind,
        ErrorKind::Expression
    );
}

#[test]
fn json_roundtrip_keeps_semantics_and_rejects_unknown_fields() {
    let flow = workflow(
        vec![scope("root", vec![], vec![("v", Expr::int(1))])],
        int_fields(&["v"]),
    );
    let json = flow.to_json().unwrap();
    assert!(prepare(Workflow::from_json(&json).unwrap(), &NodeRegistry::new()).is_ok());
    assert!(Workflow::from_json(&json.replacen('}', ",\"unknown\":1}", 1)).is_err());
}
