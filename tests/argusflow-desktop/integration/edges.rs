use super::file;
use argusflow_desktop::document::Workspace;
use serde_json::json;

#[test]
fn missing_start_or_end_never_overwrites_saved_document() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let original = file();
    let saved = workspace.save(&original, None).unwrap();
    for kind in ["start", "end"] {
        let mut draft = original.clone();
        draft.editor.nodes.remove(&format!("${kind}:root"));
        let error = workspace.save(&draft, Some(&saved.revision)).err().unwrap();
        assert!(error.contains("root") && error.contains("缺少"));
        let reopened = workspace.load("flow_test").unwrap();
        assert_eq!(reopened.revision, saved.revision);
        assert_eq!(reopened.file.definition, original.definition);
    }
}
#[test]
fn incomplete_and_multiple_output_drafts_preserve_edges_and_sides_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut draft = file();
    draft.definition["scopes"][0]["edges"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"branch", "source":{"kind":"start"}, "target":{"kind":"end"}}));
    draft.editor.edges.insert(
        "branch".into(),
        serde_json::from_value(json!({"source":"bottom", "target":"top"})).unwrap(),
    );
    let saved = workspace.save(&draft, None).unwrap();
    let reopened = workspace.load("flow_test").unwrap();
    assert_eq!(reopened.file.definition, draft.definition);
    assert_eq!(
        serde_json::to_value(&reopened.file.editor).unwrap(),
        serde_json::to_value(&draft.editor).unwrap()
    );
    draft.definition["scopes"][0]["edges"] = json!([]);
    draft.editor.edges.clear();
    workspace.save(&draft, Some(&saved.revision)).unwrap();
    assert_eq!(
        workspace.load("flow_test").unwrap().file.definition,
        draft.definition
    );
}
#[test]
fn nested_scope_missing_end_is_located_and_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut draft = file();
    draft.definition["scopes"][0]["nodes"][0]["action"] = json!({"kind":"block", "scope":"child"});
    draft.definition["scopes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"child", "nodes":[], "edges":[], "outputs":{}}));
    let error = workspace.save(&draft, None).err().unwrap();
    assert!(error.contains("child") && error.contains("缺少"));
    assert!(workspace.list().unwrap().is_empty());
}
