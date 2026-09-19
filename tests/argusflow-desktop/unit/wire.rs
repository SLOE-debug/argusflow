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

#[test]
fn generated_workflow_round_trip_keeps_timeout_and_query_interval() {
    let definition = json!({"name":"AI","root":"root","inputs":{},"outputs":{},"resources":{"window":"automation.window"},"subflows":{},"scopes":[{"id":"root","outputs":{},"nodes":[{"id":"n1","timeout_ms":{"kind":"literal","value_type":{"type":"int"},"value":{"type":"int","value":10000}},"output_bindings":{},"action":{"kind":"task","task":{"type_id":"aql.wait","version":1,"config":{"platform":"uia","query":"文档()","interval_ms":200,"condition":"unique"},"inputs":{},"resources":{"scope":"window"},"resource_outputs":{},"retry":null}}}],"edges":[{"id":"a","source":{"kind":"start"},"target":{"kind":"node","node":"n1"}},{"id":"b","source":{"kind":"node","node":"n1"},"target":{"kind":"end"}}]}]});
    let workflow = Workflow::from_json(&definition.to_string()).unwrap();
    let encoded = encode_workflow(&workflow).unwrap();
    assert_eq!(
        encoded["scopes"][0]["nodes"][0]["timeout_ms"]["value"]["value"],
        "10000"
    );
    assert_eq!(
        encoded["scopes"][0]["nodes"][0]["action"]["task"]["config"]["interval_ms"],
        "200"
    );
    let decoded = decode_workflow(&encoded).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), definition);
}
