use super::parse;
use serde_json::{Value, json};

fn endpoints() -> Value {
    json!({
        "format": "argusflow.nodes", "sourceWorkflow": "flow_test", "sourceScope": "root",
        "selected": ["$start:root", "$end:root"], "bindings": {},
        "file": {
            "id": "flow_test",
            "definition": { "name": "空流程", "inputs": {}, "outputs": {}, "resources": {}, "root": "root", "subflows": {},
                "scopes": [{"id": "root", "edges": [{"id":"direct","source":{"kind":"start"},"target":{"kind":"end"}}], "nodes": [], "outputs": {}}] },
            "editor": { "nodes": {
                "$start:root": {"x": -160.0, "y": 80.0, "label": "开始", "note": ""},
                "$end:root": {"x": 300.0, "y": 80.0, "label": "结束", "note": ""}
            }, "edges":{"direct":{"source":"right","target":"left"}}, "drafts": {} }
        }
    })
}

#[test]
fn endpoint_only_clipboard_preserves_positions_without_execution_nodes() {
    let value = endpoints();
    let parsed = parse(&value.to_string()).unwrap();
    let saved = serde_json::to_value(parsed).unwrap();
    assert_eq!(saved["file"]["editor"], value["file"]["editor"]);
    assert_eq!(saved["file"]["definition"], value["file"]["definition"]);
}

#[test]
fn partial_selection_requires_only_copied_endpoints_and_preserves_port_sides() {
    let mut value = endpoints();
    value["file"]["editor"]["edges"]["direct"] = json!({"source":"top", "target":"bottom"});
    let saved = serde_json::to_value(parse(&value.to_string()).unwrap()).unwrap();
    assert_eq!(
        saved["file"]["editor"]["edges"],
        value["file"]["editor"]["edges"]
    );
    value["selected"] = json!(["$start:root"]);
    value["file"]["editor"]["nodes"]
        .as_object_mut()
        .unwrap()
        .remove("$end:root");
    assert!(parse(&value.to_string()).is_err());
    value["file"]["definition"]["scopes"][0]["edges"] = json!([]);
    value["file"]["editor"]["edges"] = json!({});
    assert!(parse(&value.to_string()).is_ok());
    value["format"] = json!("unrelated.clipboard");
    assert!(parse(&value.to_string()).is_err());
}

#[test]
fn selection_rejects_duplicate_foreign_scope_and_missing_endpoint_layout() {
    for selection in [
        json!(["$start:root", "$start:root"]),
        json!(["$start:other", "$end:root"]),
        json!(["$start:root"]),
    ] {
        let mut value = endpoints();
        value["selected"] = selection;
        assert!(parse(&value.to_string()).is_err());
    }
    let mut value = endpoints();
    value["file"]["editor"]["nodes"]
        .as_object_mut()
        .unwrap()
        .remove("$start:root");
    assert!(parse(&value.to_string()).is_err());
}
