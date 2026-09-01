use argusflow_core::{FlowComponentDefinition, WorkflowDefinition};
use argusflow_runtime::{ComponentRegistry, expand_components};
use serde_json::json;

const COMPONENT_ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn expansion_locks_exact_version_rewrites_inputs_and_records_source_path() {
    let definition: FlowComponentDefinition = serde_json::from_value(json!({
        "schema_version": 2,
        "id": COMPONENT_ID,
        "version": "1.2.3",
        "name": "调试值",
        "inputs": [{ "key": "value", "value_type": "text" }],
        "outputs": [{
            "name": "value",
            "value": {
                "type": "ref",
                "source": { "type": "node", "node_id": "debug" },
                "pointer": "/value"
            }
        }],
        "graph": component_graph(vec![
                node("entry", "argus.start", 1, json!({})),
                node("debug", "argus.debug", 1, json!({
                    "value": {
                        "type": "ref",
                        "source": { "type": "workflow_input", "key": "value" },
                        "pointer": ""
                    }
                })),
                node("exit", "argus.end", 1, json!({}))
            ], vec![
                edge("entry-debug", "entry", "debug"),
                edge("debug-exit", "debug", "exit")
            ])
    }))
    .expect("component fixture should deserialize");
    let registry = ComponentRegistry::from_definitions([definition])
        .expect("exact component version should register");
    let workflow: WorkflowDefinition = serde_json::from_value(json!({
        "schema_version": 10,
        "id": "22222222-2222-4222-8222-222222222222",
        "name": "component expansion",
        "inputs": [],
        "variables": { "seed": "hello" },
        "permissions": { "allow": [] },
        "graph": workflow_graph(vec![
            node("start", "argus.start", 1, json!({})),
            node("instance", "argus.component", 2, json!({
                "component_id": COMPONENT_ID,
                "component_version": "1.2.3",
                "inputs": {
                    "value": {
                        "type": "ref",
                        "source": { "type": "variable", "name": "seed" },
                        "pointer": ""
                    }
                }
            })),
            node("after", "argus.debug", 1, json!({
                "value": {
                    "type": "ref",
                    "source": { "type": "node", "node_id": "instance" },
                    "pointer": "/value"
                }
            })),
            node("end", "argus.end", 1, json!({}))
        ], vec![
            edge("start-instance", "start", "instance"),
            edge("instance-after", "instance", "after"),
            edge("after-end", "after", "end")
        ])
    }))
    .expect("workflow fixture should deserialize");

    let expanded = expand_components(workflow, &registry).expect("component should expand");
    assert!(
        expanded.definition.graph.scopes[0]
            .nodes
            .iter()
            .all(|node| node.definition.type_id.as_str() != "argus.component")
    );
    let inner = expanded.definition.graph.scopes[0]
        .nodes
        .iter()
        .find(|node| node.id == "instance::debug")
        .expect("inner node should use instance namespace");
    assert_eq!(
        inner.definition.payload["value"],
        json!({
            "type": "ref",
            "source": { "type": "variable", "name": "seed" },
            "pointer": ""
        })
    );
    let source_path = expanded
        .source_map
        .get("instance::debug")
        .expect("expanded node should retain component source");
    assert_eq!(source_path.len(), 1);
    assert_eq!(source_path[0].inner_node_id, "debug");
    assert!(
        expanded.definition.graph.scopes[0]
            .edges
            .iter()
            .any(|edge| { edge.id == "start-instance" && edge.target == "instance::debug" })
    );
    assert!(
        expanded.definition.graph.scopes[0]
            .edges
            .iter()
            .any(|edge| { edge.id == "instance-after" && edge.source == "instance" })
    );
}

#[test]
fn registry_rejects_non_exact_versions() {
    let mut definition: FlowComponentDefinition = serde_json::from_value(json!({
        "schema_version": 2,
        "id": COMPONENT_ID,
        "version": "1.0.0",
        "name": "empty",
        "inputs": [],
        "outputs": [],
        "graph": component_graph(
            vec![
                node("entry", "argus.start", 1, json!({})),
                node("exit", "argus.end", 1, json!({}))
            ],
            vec![edge("entry-exit", "entry", "exit")]
        )
    }))
    .expect("component fixture should deserialize");
    definition.version = argusflow_core::FlowComponentVersion::new("latest");

    let error = ComponentRegistry::from_definitions([definition])
        .expect_err("floating component versions must be rejected");
    assert!(error.to_string().contains("exact major.minor.patch"));
}

fn node(id: &str, type_id: &str, version: u16, payload: serde_json::Value) -> serde_json::Value {
    json!({
        "id": id,
        "position": { "x": 0.0, "y": 0.0 },
        "size": { "width": 160.0, "height": 96.0 },
        "type_id": type_id,
        "version": version,
        "payload": payload,
        "output_bindings": {}
    })
}

fn workflow_graph(
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "root_scope_id": "root",
        "scopes": [{
            "id": "root",
            "parent": null,
            "boundary": { "type": "workflow", "entry_node_id": "start" },
            "nodes": nodes,
            "edges": edges
        }]
    })
}

fn component_graph(
    nodes: Vec<serde_json::Value>,
    edges: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "root_scope_id": "component-root",
        "scopes": [{
            "id": "component-root",
            "parent": null,
            "boundary": {
                "type": "component",
                "entry_node_id": "entry",
                "exit_node_id": "exit"
            },
            "nodes": nodes,
            "edges": edges
        }]
    })
}

fn edge(id: &str, source: &str, target: &str) -> serde_json::Value {
    json!({ "id": id, "source": source, "target": target, "branch": null })
}
