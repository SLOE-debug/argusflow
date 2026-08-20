//! JSON 条件谓词求值回归测试。

use argusflow_core::{ConditionOperator, ConditionPredicate};
use serde_json::{Value, json};

#[test]
fn condition_predicates_cover_json_value_shapes() {
    let variables = json!({
        "enabled": true,
        "count": 8,
        "name": "ArgusFlow Studio",
        "tags": ["flow", "rpa"],
        "settings": { "theme": "daylight" },
        "empty": []
    });

    assert!(evaluate(&variables, "/enabled", ConditionOperator::Equal, Some(json!(true))));
    assert!(evaluate(&variables, "/enabled", ConditionOperator::NotEqual, Some(json!(false))));
    assert!(evaluate(&variables, "/count", ConditionOperator::GreaterThan, Some(json!(3))));
    assert!(evaluate(&variables, "/count", ConditionOperator::GreaterThanOrEqual, Some(json!(8))));
    assert!(evaluate(&variables, "/count", ConditionOperator::LessThan, Some(json!(9))));
    assert!(evaluate(&variables, "/count", ConditionOperator::LessThanOrEqual, Some(json!(8))));
    assert!(evaluate(&variables, "/name", ConditionOperator::Contains, Some(json!("Studio"))));
    assert!(evaluate(&variables, "/tags", ConditionOperator::Contains, Some(json!("rpa"))));
    assert!(evaluate(&variables, "/settings", ConditionOperator::Contains, Some(json!("theme"))));
    assert!(evaluate(&variables, "/empty", ConditionOperator::IsEmpty, None));
    assert!(evaluate(&variables, "/tags", ConditionOperator::NotEmpty, None));
    assert!(evaluate(&variables, "/enabled", ConditionOperator::Exists, None));
    assert!(evaluate(&variables, "/missing", ConditionOperator::NotExists, None));
}

#[test]
fn condition_rejects_invalid_pointer_and_type_pairs() {
    let variables = json!({ "name": "ArgusFlow" });
    let invalid_pointer = ConditionPredicate { pointer: "name".to_owned(), operator: ConditionOperator::Exists, operand: None };
    assert!(invalid_pointer.evaluate(&variables).is_err());
    let invalid_type = ConditionPredicate { pointer: "/name".to_owned(), operator: ConditionOperator::GreaterThan, operand: Some(json!(2)) };
    assert!(invalid_type.evaluate(&variables).is_err());
}

fn evaluate(variables: &Value, pointer: &str, operator: ConditionOperator, operand: Option<Value>) -> bool {
    ConditionPredicate { pointer: pointer.to_owned(), operator, operand }.evaluate(variables).expect("predicate should evaluate")
}
