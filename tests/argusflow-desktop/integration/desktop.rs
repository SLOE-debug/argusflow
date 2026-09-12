//! 文件服务的公开契约验收。
use argusflow_desktop::document::{WorkflowFile, Workspace};
use serde_json::json;
mod edges;
#[cfg(windows)]
#[path = "../support/permissions.rs"]
mod permissions;

#[cfg(windows)]
#[test]
fn initialization_rejects_unwritable_directory_and_retries_after_permission_repair() {
    let mut directory = permissions::WriteDeniedDirectory::new().unwrap();
    let result = Workspace::initialize(directory.path());
    directory.restore().unwrap();
    let error = result.err().unwrap();
    assert!(error.contains("无法写入"), "{error}");
    assert!(Workspace::initialize(directory.path()).is_ok());
}

fn file() -> WorkflowFile {
    serde_json::from_value(json!({
        "id": "flow_test",
        "definition": { "name": "草稿", "inputs": {}, "outputs": {}, "resources": {}, "root": "root", "subflows": {},
          "scopes": [{"id":"root","edges":[{"id":"entry","source":{"kind":"start"},"target":{"kind":"node","node":"node"}},{"id":"exit","source":{"kind":"node","node":"node"},"target":{"kind":"end"}}],"nodes":[{"id":"node","timeout_ms":"18446744073709551615","action":{"kind":"wait","milliseconds":{"kind":"literal","value_type":{"type":"int"},"value":{"type":"int","value":"9007199254740993"}}},"output_bindings":{}}],"outputs":{}}]},
        "editor": { "nodes": {"node":{"x":-45.5,"y":120,"label":"等待","note":"原始说明"},"$start:root":{"x":-280,"y":120,"label":"开始","note":""},"$end:root":{"x":720,"y":120,"label":"结束","note":""}}, "edges":{"entry":{"source":"right","target":"left"},"exit":{"source":"right","target":"left"}}, "drafts": {"node:milliseconds":"尚未完成"} }
    })).unwrap()
}
#[test]
fn workspace_rejects_missing_directory() {
    assert!(Workspace::open(std::path::Path::new("Z:/argusflow-not-existing-fixture")).is_err());
}
#[test]
fn initialization_creates_persistent_directory_and_keeps_saved_documents() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("appdata").join("workflows");
    let workspace = Workspace::initialize(&path).unwrap();
    assert!(workspace.list().unwrap().is_empty());
    let saved = workspace.save(&file(), None).unwrap();
    drop(workspace);
    let reopened = Workspace::initialize(&path).unwrap();
    assert_eq!(reopened.load("flow_test").unwrap().revision, saved.revision);
    assert_eq!(reopened.list().unwrap().len(), 1);
    // 写入探针不留在用户文档旁边。
    assert_eq!(std::fs::read_dir(path).unwrap().count(), 1);
}
#[test]
fn initialization_reports_invalid_path_and_can_retry_after_repair() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workflows");
    std::fs::write(&path, "占用目录路径的文件").unwrap();
    assert!(
        Workspace::initialize(&path)
            .err()
            .unwrap()
            .contains("无法创建")
    );
    std::fs::remove_file(&path).unwrap();
    assert!(Workspace::initialize(&path).is_ok());
}
#[test]
fn malformed_document_is_reported_instead_of_disappearing_from_list() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::initialize(directory.path()).unwrap();
    let path = directory.path().join("flow_test.workflow.json");
    std::fs::write(&path, "{broken").unwrap();
    assert!(workspace.list().is_err());
    std::fs::remove_file(path).unwrap();
    assert!(workspace.list().unwrap().is_empty());
}
#[test]
fn atomic_save_reopen_preserves_integer_strings_layout_and_invalid_drafts() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let original = file();
    let saved = workspace.save(&original, None).unwrap();
    let loaded = workspace.load("flow_test").unwrap();
    assert_eq!(loaded.revision, saved.revision);
    assert_eq!(loaded.file.editor.nodes["node"].x, -45.5);
    assert_eq!(loaded.file.editor.drafts["node:milliseconds"], "尚未完成");
    assert_eq!(loaded.file.definition, original.definition);
    let mut renamed = loaded.file;
    renamed.definition["name"] = json!("新名称");
    let saved = workspace.save(&renamed, Some(&saved.revision)).unwrap();
    assert_eq!(workspace.list().unwrap()[0].name, "新名称");
    assert_eq!(
        workspace.load("flow_test").unwrap().revision,
        saved.revision
    );
}
#[test]
fn conflicts_never_replace_external_changes_or_recreate_deleted_documents() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let original = file();
    let saved = workspace.save(&original, None).unwrap();
    assert!(
        workspace
            .save(&original, None)
            .err()
            .unwrap()
            .starts_with("conflict:")
    );
    let path = directory.path().join("flow_test.workflow.json");
    let mut external = file();
    external.definition["name"] = json!("外部修改");
    std::fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();
    assert!(
        workspace
            .save(&original, Some(&saved.revision))
            .err()
            .unwrap()
            .starts_with("conflict:")
    );
    assert_eq!(
        workspace.load("flow_test").unwrap().file.definition["name"],
        "外部修改"
    );
    std::fs::remove_file(path).unwrap();
    assert!(
        workspace
            .save(&original, Some(&saved.revision))
            .err()
            .unwrap()
            .starts_with("conflict:")
    );
}
#[test]
fn malformed_identity_numeric_wire_and_nonfinite_layout_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    assert!(workspace.load("../outside").is_err());
    let mut value = file();
    value.definition["scopes"][0]["nodes"][0]["timeout_ms"] = json!(42);
    assert!(workspace.save(&value, None).is_err());
    let mut value = file();
    value.editor.nodes.get_mut("node").unwrap().x = f64::NAN;
    assert!(workspace.save(&value, None).is_err());
}

#[test]
fn editable_endpoints_roundtrip_without_changing_execution_definition() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut original = file();
    for (kind, x) in [("start", -280.0), ("end", 720.0)] {
        let mut layout = original.editor.nodes["node"].clone();
        layout.x = x;
        original
            .editor
            .nodes
            .insert(format!("${kind}:root"), layout);
    }
    workspace.save(&original, None).unwrap();
    let loaded = workspace.load("flow_test").unwrap().file;
    assert_eq!(loaded.editor.nodes["$start:root"].x, -280.0);
    assert_eq!(loaded.editor.nodes["$end:root"].x, 720.0);
    assert_eq!(loaded.definition, original.definition);
}

#[test]
fn document_deletion_checks_revision_and_rejects_paths_outside_workspace() {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let original = file();
    let saved = workspace.save(&original, None).unwrap();
    assert!(workspace.remove("../outside", &saved.revision).is_err());
    let mut renamed = original;
    renamed.definition["name"] = json!("外部更名");
    let updated = workspace.save(&renamed, Some(&saved.revision)).unwrap();
    assert!(
        workspace
            .remove("flow_test", &saved.revision)
            .unwrap_err()
            .contains("conflict:")
    );
    assert_eq!(workspace.list().unwrap().len(), 1);
    workspace.remove("flow_test", &updated.revision).unwrap();
    assert!(workspace.list().unwrap().is_empty());
    assert!(workspace.load("flow_test").is_err());
    assert!(workspace.remove("flow_test", &updated.revision).is_err());
}
