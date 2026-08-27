//! 已求值 JSON 操作数的条件比较回归测试。

use argusflow_core::ConditionOperator;
use serde_json::{Value, json};

#[test]
fn condition_operators_cover_json_value_shapes() {
    assert!(evaluate(
        json!(true),
        ConditionOperator::Equal,
        Some(json!(true))
    ));
    assert!(evaluate(
        json!(true),
        ConditionOperator::NotEqual,
        Some(json!(false))
    ));
    assert!(evaluate(
        json!(8),
        ConditionOperator::GreaterThan,
        Some(json!(3))
    ));
    assert!(evaluate(
        json!(8),
        ConditionOperator::GreaterThanOrEqual,
        Some(json!(8))
    ));
    assert!(evaluate(
        json!(8),
        ConditionOperator::LessThan,
        Some(json!(9))
    ));
    assert!(evaluate(
        json!(8),
        ConditionOperator::LessThanOrEqual,
        Some(json!(8))
    ));
    assert!(evaluate(
        json!("ArgusFlow Studio"),
        ConditionOperator::Contains,
        Some(json!("Studio"))
    ));
    assert!(evaluate(
        json!(["flow", "rpa"]),
        ConditionOperator::Contains,
        Some(json!("rpa"))
    ));
    assert!(evaluate(
        json!({ "theme": "daylight" }),
        ConditionOperator::Contains,
        Some(json!("theme"))
    ));
    assert!(evaluate(json!([]), ConditionOperator::IsEmpty, None));
    assert!(evaluate(json!(["flow"]), ConditionOperator::NotEmpty, None));
    assert!(
        ConditionOperator::Exists
            .evaluate(Some(&json!(true)), None)
            .expect("exists should evaluate")
    );
    assert!(
        ConditionOperator::NotExists
            .evaluate(None, None)
            .expect("not exists should evaluate")
    );
}

#[test]
fn condition_rejects_invalid_operand_shapes_and_types() {
    assert!(
        ConditionOperator::GreaterThan
            .evaluate(Some(&json!("ArgusFlow")), Some(&json!(2)))
            .is_err()
    );
    assert!(
        ConditionOperator::Equal
            .evaluate(Some(&json!(1)), None)
            .is_err()
    );
    assert!(
        ConditionOperator::Exists
            .evaluate(Some(&json!(1)), Some(&json!(true)))
            .is_err()
    );
}

fn evaluate(left: Value, operator: ConditionOperator, right: Option<Value>) -> bool {
    operator
        .evaluate(Some(&left), right.as_ref())
        .expect("condition should evaluate")
}
