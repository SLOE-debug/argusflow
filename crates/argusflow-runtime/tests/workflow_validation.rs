//! schema v10 工作流结构、节点与引用校验契约。

mod workflow_fixture;

use argusflow_core::{
    AqlQuery, AutomationTarget, BackendKind, BackendPolicy, CommandOperation, CommandRunner,
    ControlPortId, FlowScope, FlowScopeBoundary, FlowScopeParent, NodeEnvelope, Position,
    ResourceRef, Size, TargetLocator, TargetScope, TargetWaitPolicy, UiExecutionPolicy,
    UiOperation, ValueExpr, WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::{ValidationIssueCode, validate_workflow};
use serde_json::json;

use workflow_fixture::{
    WorkflowNodeKind, condition_workflow, demo_workflow, edge, node, test_application_spec,
};

#[test]
fn valid_linear_workflow_passes_validation() {
    assert!(validate_workflow(&demo_workflow(1)).valid);
}

#[test]
fn validation_accepts_one_level_bounded_loop_and_rejects_an_unbounded_cycle() {
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes[2].definition = WorkflowNodeKind::Loop {
        body_scope_id: "body".to_owned(),
        max_iterations: 3,
        timeout_ms: 1_000,
        interval_ms: 0,
    }
    .into();
    workflow.graph.scopes[0].nodes[2].size = Size {
        width: 420.0,
        height: 240.0,
    };
    workflow.graph.scopes[0]
        .nodes
        .retain(|node| node.id != "log");
    workflow.graph.scopes[0].edges = vec![
        edge("start", "delay"),
        WorkflowEdge {
            id: "loop-completed".to_owned(),
            source: "delay".to_owned(),
            target: "end".to_owned(),
            branch: Some(ControlPortId::new("completed")),
        },
        WorkflowEdge {
            id: "loop-exhausted".to_owned(),
            source: "delay".to_owned(),
            target: "end".to_owned(),
            branch: Some(ControlPortId::new("exhausted")),
        },
    ];
    workflow.graph.scopes.push(FlowScope {
        id: "body".to_owned(),
        parent: Some(FlowScopeParent {
            scope_id: "root".to_owned(),
            node_id: "delay".to_owned(),
        }),
        boundary: FlowScopeBoundary::Loop {
            entry_node_id: "body-entry".to_owned(),
            continue_node_id: "body-continue".to_owned(),
            complete_node_id: "body-complete".to_owned(),
        },
        nodes: vec![
            boundary_node("body-entry", "argus.loop.entry"),
            node(
                "body-log",
                220.0,
                WorkflowNodeKind::Log {
                    message: "body".to_owned(),
                },
            ),
            boundary_node("body-continue", "argus.loop.continue"),
            boundary_node("body-complete", "argus.loop.complete"),
        ],
        edges: vec![
            edge("body-entry", "body-log"),
            edge("body-log", "body-continue"),
        ],
    });
    let report = validate_workflow(&workflow);
    assert!(report.valid, "{:#?}", report.issues);

    workflow.graph.scopes[1]
        .edges
        .push(edge("body-continue", "body-log"));
    assert_has_issue(&workflow, ValidationIssueCode::CycleDetected);
}

/// 创建结构化 While 子作用域中的固定边界节点。
fn boundary_node(id: &str, type_id: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x: 0.0, y: 0.0 },
        size: Size {
            width: 142.0,
            height: 52.0,
        },
        definition: NodeEnvelope::new(type_id, 1, json!({})),
        output_bindings: Default::default(),
    }
}

#[test]
fn validation_rejects_invalid_aql_before_execution() {
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Ui {
        operation: UiOperation::Click {
            target: AutomationTarget::query(AqlQuery::v3(r#"button[name="保存"]"#)),
        },
    }
    .into();

    let report = validate_workflow(&workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == ValidationIssueCode::InvalidAqlQuery)
        .expect("invalid AQL should produce a node issue");
    assert_eq!(issue.node_id.as_deref(), Some("log"));
    assert!(issue.message.contains("CSS"), "{}", issue.message);
}

#[test]
fn ui_payload_v5_rejects_waiting_on_coordinates() {
    let coordinate_operation = UiOperation::Click {
        target: AutomationTarget::coordinate(20, 40),
    };
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
        "argus.ui",
        5,
        json!({
            "operation": coordinate_operation,
            "execution": UiExecutionPolicy {
                target_wait: TargetWaitPolicy::bounded(5_000, 100),
            },
        }),
    );

    assert_has_issue(&workflow, ValidationIssueCode::InvalidTargetWaitPolicy);
}

#[test]
fn ui_payload_v5_rejects_removed_postconditions_and_prior_payload_versions() {
    let operation = json!({
        "type": "press_key",
        "target": {
            "scope": { "type": "current" },
            "locator": { "type": "focused" },
            "backend_policy": {
                "allow": ["send_input"],
                "deny": [],
                "prefer": ["send_input"]
            }
        },
        "chord": { "key": { "type": "enter" }, "modifiers": [] }
    });
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
        "argus.ui",
        5,
        json!({
            "operation": operation,
            "execution": {
                "target_wait": { "mode": "none", "timeout_ms": 0, "poll_interval_ms": 0 },
                "postcondition": {
                    "type": "match_removed",
                    "query": {
                    "language_version": 3,
                        "source": "nearest(anchor = viewport_edge(side = bottom), target = text(name = $message), direction = any, index = 1)",
                        "bindings": {
                            "message": { "type": "literal", "value": "你好" }
                        }
                    },
                    "stable_context": [{
                        "language_version": 3,
                        "source": "nearest(anchor = viewport_corner(position = top_right), target = text(name = $contact), direction = any, index = 1)",
                        "bindings": {
                            "contact": { "type": "literal", "value": "联系人" }
                        }
                    }]
                }
            }
        }),
    );
    let removed_report = validate_workflow(&workflow);
    assert!(
        removed_report
            .issues
            .iter()
            .any(|issue| issue.code == ValidationIssueCode::InvalidNodeDefinition),
        "{:#?}",
        removed_report.issues,
    );

    let mut prior_version = demo_workflow(1);
    prior_version.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
        "argus.ui",
        4,
        json!({
            "operation": operation,
            "execution": {
                "target_wait": { "mode": "none", "timeout_ms": 0, "poll_interval_ms": 0 }
            }
        }),
    );
    let version_report = validate_workflow(&prior_version);
    assert!(
        version_report
            .issues
            .iter()
            .any(|issue| issue.code == ValidationIssueCode::InvalidNodeDefinition),
        "{:#?}",
        version_report.issues,
    );
}

#[test]
fn validation_rejects_unknown_types_and_invalid_registered_payloads() {
    let mut unknown = demo_workflow(1);
    unknown.graph.scopes[0].nodes[1].definition =
        NodeEnvelope::new("plugin.database.query", 1, json!({}));
    assert_has_issue(&unknown, ValidationIssueCode::UnknownNodeType);

    let mut invalid_payload = demo_workflow(1);
    invalid_payload.graph.scopes[0].nodes[1].definition =
        NodeEnvelope::new("argus.log", 1, json!({ "unexpected": "missing message" }));
    assert_has_issue(&invalid_payload, ValidationIssueCode::InvalidNodeDefinition);
}

#[test]
fn validation_rejects_duplicate_ids_unknown_edges_cycles_branches_and_unreachable_nodes() {
    let mut duplicate = demo_workflow(1);
    duplicate.graph.scopes[0].nodes[1].id = "start".to_owned();
    assert_has_issue(&duplicate, ValidationIssueCode::DuplicateNodeId);

    let mut unknown_edge = demo_workflow(1);
    unknown_edge.graph.scopes[0].edges[0].target = "missing".to_owned();
    assert_has_issue(&unknown_edge, ValidationIssueCode::UnknownEdgeEndpoint);

    let mut duplicate_edge = demo_workflow(1);
    duplicate_edge.graph.scopes[0].edges[1].id = duplicate_edge.graph.scopes[0].edges[0].id.clone();
    assert_has_issue(&duplicate_edge, ValidationIssueCode::DuplicateEdgeId);

    let mut cycle = demo_workflow(1);
    cycle.graph.scopes[0].edges.push(WorkflowEdge {
        id: "cycle".to_owned(),
        source: "end".to_owned(),
        target: "start".to_owned(),
        branch: None,
    });
    assert_has_issue(&cycle, ValidationIssueCode::CycleDetected);

    let mut branch = demo_workflow(1);
    branch.graph.scopes[0].nodes.insert(
        2,
        WorkflowNode {
            id: "extra".to_owned(),
            position: Position { x: 400.0, y: 120.0 },
            size: argusflow_core::Size {
                width: 142.0,
                height: 52.0,
            },
            definition: WorkflowNodeKind::Log {
                message: "branch".to_owned(),
            }
            .into(),
            output_bindings: Default::default(),
        },
    );
    branch.graph.scopes[0].edges.push(WorkflowEdge {
        id: "branch".to_owned(),
        source: "start".to_owned(),
        target: "extra".to_owned(),
        branch: None,
    });
    assert_has_issue(&branch, ValidationIssueCode::InvalidNodeDegree);

    let mut unreachable = demo_workflow(1);
    unreachable.graph.scopes[0].nodes.push(WorkflowNode {
        id: "orphan".to_owned(),
        position: Position { x: 0.0, y: 200.0 },
        size: argusflow_core::Size {
            width: 142.0,
            height: 52.0,
        },
        definition: WorkflowNodeKind::Log {
            message: "orphan".to_owned(),
        }
        .into(),
        output_bindings: Default::default(),
    });
    assert_has_issue(&unreachable, ValidationIssueCode::UnreachableNode);
}

#[test]
fn validation_requires_exactly_one_start_and_end() {
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0]
        .nodes
        .retain(|node| node.definition.type_id.as_str() != "argus.end");
    assert_has_issue(&workflow, ValidationIssueCode::InvalidEndCount);

    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes.push(WorkflowNode {
        id: "another-start".to_owned(),
        position: Position { x: 0.0, y: 100.0 },
        size: argusflow_core::Size {
            width: 142.0,
            height: 52.0,
        },
        definition: WorkflowNodeKind::Start.into(),
        output_bindings: Default::default(),
    });
    assert_has_issue(&workflow, ValidationIssueCode::InvalidStartCount);
}

#[test]
fn validation_accepts_a_condition_dag_with_both_branches() {
    assert!(validate_workflow(&condition_workflow(true)).valid);
}

#[test]
fn validation_rejects_an_application_resource_that_does_not_dominate_its_consumer() {
    let mut workflow = condition_workflow(true);
    workflow.graph.scopes[0]
        .nodes
        .retain(|node| node.id != "true-log" && node.id != "false-log");
    workflow.graph.scopes[0].nodes.insert(
        2,
        node(
            "application",
            320.0,
            WorkflowNodeKind::Application {
                spec: test_application_spec(),
            },
        ),
    );
    workflow.graph.scopes[0].nodes.insert(
        3,
        node(
            "consumer",
            480.0,
            WorkflowNodeKind::Ui {
                operation: UiOperation::Click {
                    target: AutomationTarget {
                        scope: TargetScope::Application {
                            resource: ResourceRef {
                                producer_node_id: "application".to_owned(),
                                output_name: "session".to_owned(),
                            },
                        },
                        locator: TargetLocator::Query {
                            query: AqlQuery::v3("button(name = \"保存\")"),
                        },
                        backend_policy: BackendPolicy::default(),
                    },
                },
            },
        ),
    );
    workflow.graph.scopes[0].edges = vec![
        edge("start", "condition"),
        WorkflowEdge {
            id: "condition-true".to_owned(),
            source: "condition".to_owned(),
            target: "application".to_owned(),
            branch: Some(ControlPortId::new("true")),
        },
        WorkflowEdge {
            id: "condition-false".to_owned(),
            source: "condition".to_owned(),
            target: "consumer".to_owned(),
            branch: Some(ControlPortId::new("false")),
        },
        edge("application", "consumer"),
        edge("consumer", "end"),
    ];

    assert_has_issue(&workflow, ValidationIssueCode::ReferenceNotDominating);
}

#[test]
fn validation_rejects_browser_cdp_for_a_desktop_application_resource() {
    let resource = ResourceRef {
        producer_node_id: "application".to_owned(),
        output_name: "session".to_owned(),
    };
    let target = AutomationTarget {
        scope: TargetScope::Application { resource },
        locator: TargetLocator::Query {
            query: AqlQuery::v3("button(name = \"Save\")"),
        },
        backend_policy: BackendPolicy::only(BackendKind::BrowserCdp),
    };
    let workflow = workflow_fixture::workflow_definition(
        "Application backend validation",
        vec![
            node("start", 0.0, WorkflowNodeKind::Start),
            node(
                "application",
                200.0,
                WorkflowNodeKind::Application {
                    spec: test_application_spec(),
                },
            ),
            node(
                "ui",
                400.0,
                WorkflowNodeKind::Ui {
                    operation: UiOperation::Click { target },
                },
            ),
            node("end", 600.0, WorkflowNodeKind::End),
        ],
        vec![
            edge("start", "application"),
            edge("application", "ui"),
            edge("ui", "end"),
        ],
    );

    assert_has_issue(&workflow, ValidationIssueCode::InvalidBackendPolicy);
}

#[test]
fn validation_requires_explicit_command_permissions() {
    let mut workflow = demo_workflow(1);
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Command {
        operation: CommandOperation {
            runner: CommandRunner::Direct,
            program: Some(ValueExpr::text(r"C:\Windows\System32\whoami.exe")),
            arguments: Vec::new(),
            script: None,
            working_directory: None,
            environment: Vec::new(),
            stdin: None,
            timeout_ms: 30_000,
            accepted_exit_codes: vec![0],
            max_stdout_bytes: 1_048_576,
            max_stderr_bytes: 1_048_576,
        },
    }
    .into();

    assert_has_issue(&workflow, ValidationIssueCode::CommandPermissionDenied);
    workflow.permissions = WorkflowPermissions::direct_command_only();
    assert!(validate_workflow(&workflow).valid);
}

/// 断言工作流包含指定稳定问题码。
fn assert_has_issue(workflow: &WorkflowDefinition, code: ValidationIssueCode) {
    let report = validate_workflow(workflow);
    assert!(report.issues.iter().any(|issue| issue.code == code));
}
