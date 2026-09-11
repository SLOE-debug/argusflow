use super::*;
use argusflow_runtime::{NodeRegistry, RunInputs, RunOptions, WorkflowEngine, prepare};
use argusflow_workflow::Value;
use serde_json::json;

fn file() -> WorkflowFile {
    serde_json::from_value(json!({
        "format_version":1,"id":"flow","definition":{"schema_version":1,"name":"返回","inputs":{},"outputs":{"answer":{"type":"int"}},"resources":{},"root":"root","subflows":{},
        "scopes":[{"id":"root","entry":"return","nodes":[{"id":"return","next":null,"timeout_ms":null,"action":{"kind":"return","values":{"answer":{"kind":"literal","value_type":{"type":"int"},"value":{"type":"int","value":"42"}}}},"output_bindings":{}}],"outputs":{"answer":{"kind":"literal","value_type":{"type":"int"},"value":{"type":"int","value":"9007199254740993"}}}}]},
        "editor":{"nodes":{"return":{"x":0,"y":0,"label":"返回","note":""}},"drafts":{}}
    })).unwrap()
}
#[tokio::test]
async fn return_excludes_unreachable_normal_output_without_destroying_editor_draft() {
    let file = file();
    let workflow = compile(&file).unwrap();
    assert!(workflow.scopes[0].outputs.is_empty());
    assert!(file.definition["scopes"][0]["outputs"]["answer"].is_object());
    let plan = prepare(workflow, &NodeRegistry::new()).unwrap();
    let result = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(result.outputs["answer"], Value::Int(42));
}
#[test]
fn unfinished_current_field_never_runs_previous_value() {
    let mut file = file();
    file.editor
        .drafts
        .insert("return:return.answer".into(), "-".into());
    assert!(compile(&file).is_err());
}
