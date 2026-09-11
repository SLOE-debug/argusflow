use super::*;
use argusflow_workflow::{Value as Data, Values};
use serde_json::json;
#[test]
fn nested_integer_endpoints_round_trip_without_number_coercion() {
    let values = Values::from([
        ("min".into(), Data::Int(i64::MIN)),
        ("max".into(), Data::Int(i64::MAX)),
        ("list".into(), Data::List(vec![Data::Int(9007199254740993)])),
        (
            "record".into(),
            Data::Record(Values::from([
                ("timeout_ms".into(), Data::Int(i64::MAX)),
                ("config".into(), Data::Int(i64::MIN)),
            ])),
        ),
    ]);
    let encoded = encode_values(&values).unwrap();
    assert_eq!(encoded["min"]["value"], i64::MIN.to_string());
    assert_eq!(encoded["max"]["value"], i64::MAX.to_string());
    assert_eq!(decode_inputs(encoded).unwrap(), values);
    assert!(decode_inputs(json!({"x":{"type":"int","value":9007199254740993_i64}})).is_err());
    assert!(decode_inputs(json!({"x":{"type":"int","value":"9223372036854775808"}})).is_err());
}
