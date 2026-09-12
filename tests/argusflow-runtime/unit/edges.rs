use super::support::*;
use argusflow_runtime::{NodeRegistry, prepare, prepare_bundle};
use argusflow_workflow::*;
use std::collections::BTreeMap;

fn linear() -> Workflow {
    workflow(
        vec![scope(
            "root",
            vec![
                node(
                    "a",
                    Action::Wait {
                        milliseconds: Expr::int(0),
                    },
                ),
                node(
                    "b",
                    Action::Wait {
                        milliseconds: Expr::int(0),
                    },
                ),
            ],
            vec![],
        )],
        Fields::new(),
    )
}
fn edge(id: &str, source: EdgeEndpoint, target: EdgeEndpoint) -> WorkflowEdge {
    WorkflowEdge {
        id: id.into(),
        source,
        target,
    }
}
#[test]
fn draft_branches_and_merges_are_structural_but_never_choose_an_exit_at_prepare() {
    let mut source = linear();
    source.scopes[0]
        .edges
        .push(edge("branch", EdgeEndpoint::node("a"), EdgeEndpoint::End));
    assert!(ScopeGraph::new(&source.scopes[0]).is_ok());
    let error = prepare(source, &NodeRegistry::new())
        .err()
        .unwrap()
        .remove(0);
    assert_eq!(error.scope.as_deref(), Some("root"));
    assert_eq!(error.node.as_deref(), Some("a"));
    assert!(error.message.contains("多个出口"));
}
#[test]
fn called_workflow_with_multiple_outputs_blocks_the_entire_bundle() {
    let mut child = linear();
    child.scopes[0]
        .edges
        .push(edge("branch", EdgeEndpoint::node("a"), EdgeEndpoint::End));
    let root = workflow(
        vec![scope(
            "root",
            vec![node(
                "invoke",
                Action::CallWorkflow {
                    workflow: WorkflowId("child".into()),
                    inputs: BTreeMap::new(),
                    resources: BTreeMap::new(),
                },
            )],
            vec![],
        )],
        Fields::new(),
    );
    let source = WorkflowBundle {
        root: WorkflowId("parent".into()),
        workflows: BTreeMap::from([
            (WorkflowId("parent".into()), root),
            (WorkflowId("child".into()), child),
        ]),
    };
    let error = prepare_bundle(source, &NodeRegistry::new())
        .err()
        .unwrap()
        .remove(0);
    assert_eq!(error.workflow, Some(WorkflowId("child".into())));
    assert_eq!(error.node.as_deref(), Some("a"));
    assert!(error.message.contains("多个出口"));
}
#[test]
fn entry_missing_unreachable_nodes_and_normal_path_without_end_are_not_executable() {
    for remove in [0, 1, 2] {
        let mut source = linear();
        source.scopes[0].edges.remove(remove);
        assert!(ScopeGraph::new(&source.scopes[0]).is_ok());
        assert!(prepare(source, &NodeRegistry::new()).is_err());
    }
    let mut source = linear();
    source.scopes[0].edges = vec![edge("direct", EdgeEndpoint::Start, EdgeEndpoint::End)];
    assert!(
        prepare(source, &NodeRegistry::new()).err().unwrap()[0]
            .message
            .contains("不可达")
    );
    assert!(prepare(linear(), &NodeRegistry::new()).is_ok());
    assert!(
        prepare(
            workflow(vec![scope("root", vec![], vec![])], Fields::new()),
            &NodeRegistry::new()
        )
        .is_ok()
    );
}
#[test]
fn graph_rejects_duplicate_self_loop_cycles_foreign_nodes_and_terminal_outputs() {
    for (from, to) in [
        (EdgeEndpoint::node("a"), EdgeEndpoint::node("b")),
        (EdgeEndpoint::node("a"), EdgeEndpoint::node("a")),
        (EdgeEndpoint::node("b"), EdgeEndpoint::node("a")),
        (EdgeEndpoint::node("a"), EdgeEndpoint::node("foreign")),
        (EdgeEndpoint::End, EdgeEndpoint::node("a")),
        (EdgeEndpoint::node("b"), EdgeEndpoint::Start),
    ] {
        let mut source = linear();
        source.scopes[0].edges.push(edge("invalid", from, to));
        assert!(ScopeGraph::new(&source.scopes[0]).is_err());
    }
    let mut source = workflow(
        vec![scope(
            "root",
            vec![node(
                "return",
                Action::Return {
                    values: BTreeMap::new(),
                },
            )],
            vec![],
        )],
        Fields::new(),
    );
    assert!(prepare(source.clone(), &NodeRegistry::new()).is_ok());
    source.scopes[0].edges.push(edge(
        "invalid",
        EdgeEndpoint::node("return"),
        EdgeEndpoint::End,
    ));
    assert!(ScopeGraph::new(&source.scopes[0]).is_err());
}
#[test]
fn graph_structure_rejects_undeclared_fields() {
    let value = serde_json::to_value(linear()).unwrap();
    for key in ["entry", "next"] {
        let mut value = value.clone();
        if key == "entry" {
            value["scopes"][0][key] = serde_json::json!("a");
        } else {
            value["scopes"][0]["nodes"][0][key] = serde_json::json!("b");
        }
        assert!(serde_json::from_value::<Workflow>(value).is_err());
    }
}
