use super::{support::*, tasks::*};
use argusflow_runtime::*;
use argusflow_workflow::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

fn external(id: &str, target: &str) -> Node {
    node(
        id,
        Action::CallWorkflow {
            workflow: WorkflowId(target.into()),
            inputs: BTreeMap::new(),
            resources: BTreeMap::new(),
        },
    )
}
fn bundle(root: Workflow, child: Workflow) -> WorkflowBundle {
    WorkflowBundle {
        root: WorkflowId("parent".into()),
        workflows: BTreeMap::from([
            (WorkflowId("parent".into()), root),
            (WorkflowId("child".into()), child),
        ]),
    }
}
#[tokio::test]
async fn independent_root_and_internal_subflow_are_isolated_and_repeatable() {
    let parent = workflow(
        vec![scope(
            "root",
            vec![
                let_int("x", "x", Expr::int(100)),
                external("one", "child"),
                external("two", "child"),
            ],
            vec![
                ("parent", Expr::var("x")),
                ("first", output("one", "answer")),
                ("second", output("two", "answer")),
            ],
        )],
        int_fields(&["parent", "first", "second"]),
    );
    let mut child = workflow(
        vec![
            scope(
                "root",
                vec![
                    let_int("x", "x", Expr::int(2)),
                    call("internal", "increment", vec![]),
                ],
                vec![("answer", output("internal", "answer"))],
            ),
            scope(
                "sub",
                vec![assign("increment", "x", add(Expr::var("x"), Expr::int(3)))],
                vec![("answer", Expr::var("x"))],
            ),
        ],
        int_fields(&["answer"]),
    );
    child.subflows.insert(
        "increment".into(),
        Subflow {
            scope: "sub".into(),
            inputs: Fields::new(),
            outputs: int_fields(&["answer"]),
            resources: BTreeMap::new(),
        },
    );
    let registry = NodeRegistry::new();
    let standalone = prepare_bundle(
        WorkflowBundle {
            root: WorkflowId("child".into()),
            workflows: BTreeMap::from([(WorkflowId("child".into()), child.clone())]),
        },
        &registry,
    )
    .unwrap();
    let engine = WorkflowEngine::new();
    let standalone = engine
        .start(standalone, RunInputs::default(), RunOptions::default())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_output(&standalone, "answer", 5);
    let plan = prepare_bundle(bundle(parent, child), &registry).unwrap();
    let result = engine
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_output(&result, "parent", 100);
    assert_output(&result, "first", 5);
    assert_output(&result, "second", 5);
}
#[tokio::test]
async fn explicit_inputs_return_only_outputs_and_dependency_snapshot_are_frozen() {
    let mut call = external("invoke", "child");
    if let Action::CallWorkflow { inputs, .. } = &mut call.action {
        inputs.insert("number".into(), Expr::int(9007199254740993));
    }
    let parent = workflow(
        vec![scope(
            "root",
            vec![call],
            vec![("answer", output("invoke", "answer"))],
        )],
        int_fields(&["answer"]),
    );
    let mut child = workflow(
        vec![scope(
            "root",
            vec![node(
                "return",
                Action::Return {
                    values: BTreeMap::from([(
                        "answer".into(),
                        Expr::Input {
                            name: "number".into(),
                        },
                    )]),
                },
            )],
            vec![],
        )],
        int_fields(&["answer"]),
    );
    child.inputs = int_fields(&["number"]);
    let mut source = bundle(parent, child);
    let plan = prepare_bundle(source.clone(), &NodeRegistry::new()).unwrap();
    source
        .workflows
        .get_mut(&WorkflowId("child".into()))
        .unwrap()
        .inputs
        .clear();
    assert!(prepare_bundle(source, &NodeRegistry::new()).is_err());
    let result = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_output(&result, "answer", 9007199254740993);
}
#[test]
fn cycles_missing_dependencies_and_ports_are_rejected_with_workflow_location() {
    let parent = workflow(
        vec![scope("root", vec![external("call", "child")], vec![])],
        Fields::new(),
    );
    let child = workflow(
        vec![scope("root", vec![external("recurse", "parent")], vec![])],
        Fields::new(),
    );
    let source = bundle(parent.clone(), child);
    assert!(
        prepare_bundle(source, &NodeRegistry::new()).err().unwrap()[0]
            .message
            .contains("递归")
    );
    let missing = WorkflowBundle {
        root: WorkflowId("parent".into()),
        workflows: BTreeMap::from([(WorkflowId("parent".into()), parent.clone())]),
    };
    let error = prepare_bundle(missing, &NodeRegistry::new())
        .err()
        .unwrap()
        .remove(0);
    assert_eq!(error.workflow, Some(WorkflowId("parent".into())));
    assert_eq!(error.node.as_deref(), Some("call"));
    let bad = workflow(
        vec![scope(
            "root",
            vec![let_int("invalid", "x", Expr::var("missing"))],
            vec![],
        )],
        Fields::new(),
    );
    let error = prepare_bundle(bundle(parent, bad), &NodeRegistry::new())
        .err()
        .unwrap()
        .remove(0);
    assert_eq!(error.workflow, Some(WorkflowId("child".into())));
    assert_eq!(error.node.as_deref(), Some("invalid"));
}
#[tokio::test]
async fn failure_in_child_stops_parent_and_cleans_parent_owned_resource_once() {
    let probe = Arc::new(Probe::default());
    let mut registry = NodeRegistry::new();
    for (id, mode) in [
        ("acquire", Mode::Acquire),
        ("use", Mode::Use),
        ("business", Mode::EffectSuccess),
    ] {
        registry
            .register(Arc::new(Compiler {
                id,
                mode,
                probe: probe.clone(),
            }))
            .unwrap();
    }
    let mut acquire = task("acquire", "acquire");
    if let Action::Task { task } = &mut acquire.action {
        task.resource_outputs.insert("out".into(), "owned".into());
    }
    let mut call = external("invoke", "child");
    if let Action::CallWorkflow { resources, .. } = &mut call.action {
        resources.insert("borrow".into(), "owned".into());
    }
    let parent = workflow(
        vec![scope(
            "root",
            vec![acquire, call, task("must-not-run", "business")],
            vec![],
        )],
        Fields::new(),
    );
    let mut use_resource = task("use", "use");
    if let Action::Task { task } = &mut use_resource.action {
        task.resources.insert("in".into(), "borrow".into());
    }
    let mut child = workflow(
        vec![scope(
            "root",
            vec![
                use_resource,
                node(
                    "failure",
                    Action::Fail {
                        code: "child_failure".into(),
                    },
                ),
            ],
            vec![],
        )],
        Fields::new(),
    );
    child
        .resources
        .insert("borrow".into(), "test.resource".into());
    let plan = prepare_bundle(bundle(parent, child), &registry).unwrap();
    let result = WorkflowEngine::new()
        .start(plan, RunInputs::default(), RunOptions::default())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(result.status, RunStatus::Failed);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(probe.cleaned.load(Ordering::SeqCst), 1);
    let error = result.error.as_ref().unwrap();
    assert_eq!(error.message, "child_failure");
    assert_eq!(
        error.path.last().unwrap().workflow,
        Some(WorkflowId("child".into()))
    );
    assert_eq!(
        error.path.first().unwrap().workflow,
        Some(WorkflowId("parent".into()))
    );
}
#[tokio::test]
async fn cancellation_and_total_timeout_cross_independent_calls() {
    for cancel in [true, false] {
        let parent = workflow(
            vec![scope("root", vec![external("invoke", "child")], vec![])],
            Fields::new(),
        );
        let child = workflow(
            vec![scope(
                "root",
                vec![node(
                    "wait",
                    Action::Wait {
                        milliseconds: Expr::int(10000),
                    },
                )],
                vec![],
            )],
            Fields::new(),
        );
        let plan = prepare_bundle(bundle(parent, child), &NodeRegistry::new()).unwrap();
        let mut handle = WorkflowEngine::new()
            .start(
                plan,
                RunInputs::default(),
                RunOptions {
                    timeout: Duration::from_millis(40),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut events = handle.subscribe();
        loop {
            if let EventRead::Event(event) = events.recv().await
                && event
                    .path
                    .last()
                    .is_some_and(|at| at.node.as_deref() == Some("wait"))
            {
                break;
            }
        }
        if cancel {
            handle.cancel();
        }
        let result = tokio::time::timeout(Duration::from_secs(2), handle.wait())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.status,
            if cancel {
                RunStatus::Cancelled
            } else {
                RunStatus::TimedOut
            }
        );
    }
}
