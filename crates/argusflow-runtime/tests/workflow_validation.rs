//! schema v9 工作流结构、节点与引用校验契约。

mod workflow_fixture;

use argusflow_core::{
    AcquirePolicy, AqlQuery, AutomationTarget, BackendKind, BackendPolicy, CommandOperation,
    CommandRunner, ControlPortId, NodeEnvelope, Position, ResourceRef, TargetLocator, TargetScope,
    TargetWaitPolicy, UiExecutionPolicy, UiOperation, ValueExpr, ValueSource, WorkflowDefinition,
    WorkflowEdge, WorkflowInputDefinition, WorkflowInputType, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::{ValidationIssueCode, validate_workflow};
use serde_json::json;
use uuid::Uuid;

use workflow_fixture::{
    WorkflowNodeKind, condition_workflow, demo_workflow, edge, no_permissions, node,
    test_application_spec,
};

#[test]
fn valid_linear_workflow_passes_validation() {
    assert!(validate_workflow(&demo_workflow(1)).valid);
}

#[test]
fn validation_accepts_one_level_bounded_loop_and_rejects_an_unbounded_cycle() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[2].definition = WorkflowNodeKind::Loop {
        max_iterations: 3,
        timeout_ms: 1_000,
        interval_ms: 0,
    }
    .into();
    workflow.edges = vec![
        edge("start", "delay"),
        WorkflowEdge {
            id: "loop-body".to_owned(),
            source: "delay".to_owned(),
            target: "log".to_owned(),
            branch: Some(ControlPortId::new("iterate")),
        },
        edge("log", "delay"),
        WorkflowEdge {
            id: "loop-exit".to_owned(),
            source: "delay".to_owned(),
            target: "end".to_owned(),
            branch: Some(ControlPortId::new("exhausted")),
        },
    ];
    let report = validate_workflow(&workflow);
    assert!(report.valid, "{:#?}", report.issues);

    workflow.nodes[2].definition = WorkflowNodeKind::Delay { milliseconds: 1 }.into();
    assert_has_issue(&workflow, ValidationIssueCode::CycleDetected);
}

#[test]
fn validation_rejects_invalid_aql_before_execution() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = WorkflowNodeKind::Ui {
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
    workflow.nodes[1].definition = NodeEnvelope::new(
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
    workflow.nodes[1].definition = NodeEnvelope::new(
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
    prior_version.nodes[1].definition = NodeEnvelope::new(
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
    unknown.nodes[1].definition = NodeEnvelope::new("plugin.database.query", 1, json!({}));
    assert_has_issue(&unknown, ValidationIssueCode::UnknownNodeType);

    let mut invalid_payload = demo_workflow(1);
    invalid_payload.nodes[1].definition =
        NodeEnvelope::new("argus.log", 1, json!({ "unexpected": "missing message" }));
    assert_has_issue(&invalid_payload, ValidationIssueCode::InvalidNodeDefinition);
}

#[test]
fn validation_rejects_duplicate_ids_unknown_edges_cycles_branches_and_unreachable_nodes() {
    let mut duplicate = demo_workflow(1);
    duplicate.nodes[1].id = "start".to_owned();
    assert_has_issue(&duplicate, ValidationIssueCode::DuplicateNodeId);

    let mut unknown_edge = demo_workflow(1);
    unknown_edge.edges[0].target = "missing".to_owned();
    assert_has_issue(&unknown_edge, ValidationIssueCode::UnknownEdgeEndpoint);

    let mut duplicate_edge = demo_workflow(1);
    duplicate_edge.edges[1].id = duplicate_edge.edges[0].id.clone();
    assert_has_issue(&duplicate_edge, ValidationIssueCode::DuplicateEdgeId);

    let mut cycle = demo_workflow(1);
    cycle.edges.push(WorkflowEdge {
        id: "cycle".to_owned(),
        source: "end".to_owned(),
        target: "start".to_owned(),
        branch: None,
    });
    assert_has_issue(&cycle, ValidationIssueCode::CycleDetected);

    let mut branch = demo_workflow(1);
    branch.nodes.insert(
        2,
        WorkflowNode {
            id: "extra".to_owned(),
            position: Position { x: 400.0, y: 120.0 },
            definition: WorkflowNodeKind::Log {
                message: "branch".to_owned(),
            }
            .into(),
            output_bindings: Default::default(),
        },
    );
    branch.edges.push(WorkflowEdge {
        id: "branch".to_owned(),
        source: "start".to_owned(),
        target: "extra".to_owned(),
        branch: None,
    });
    assert_has_issue(&branch, ValidationIssueCode::InvalidNodeDegree);

    let mut unreachable = demo_workflow(1);
    unreachable.nodes.push(WorkflowNode {
        id: "orphan".to_owned(),
        position: Position { x: 0.0, y: 200.0 },
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
    workflow
        .nodes
        .retain(|node| node.definition.type_id.as_str() != "argus.end");
    assert_has_issue(&workflow, ValidationIssueCode::InvalidEndCount);

    let mut workflow = demo_workflow(1);
    workflow.nodes.push(WorkflowNode {
        id: "another-start".to_owned(),
        position: Position { x: 0.0, y: 100.0 },
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
    workflow
        .nodes
        .retain(|node| node.id != "true-log" && node.id != "false-log");
    workflow.nodes.insert(
        2,
        node(
            "application",
            320.0,
            WorkflowNodeKind::Application {
                spec: test_application_spec(),
            },
        ),
    );
    workflow.nodes.insert(
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
    workflow.edges = vec![
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
    let workflow = WorkflowDefinition {
        schema_version: 9,
        id: Uuid::new_v4(),
        name: "Application backend validation".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: no_permissions(),
        nodes: vec![
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
        edges: vec![
            edge("start", "application"),
            edge("application", "ui"),
            edge("ui", "end"),
        ],
    };

    assert_has_issue(&workflow, ValidationIssueCode::InvalidBackendPolicy);
}

#[test]
fn validation_requires_explicit_command_permissions() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = WorkflowNodeKind::Command {
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

#[test]
fn validation_uses_input_declarations_instead_of_persisted_variables() {
    let mut workflow = demo_workflow(1);
    workflow.inputs = vec![WorkflowInputDefinition {
        key: "secret".to_owned(),
        value_type: WorkflowInputType::Text,
    }];
    workflow.variables = json!({ "secret": 42 });
    workflow.nodes[1].definition = WorkflowNodeKind::Debug {
        value: ValueExpr::Ref {
            source: ValueSource::WorkflowInput {
                key: "secret".to_owned(),
            },
            pointer: String::new(),
        },
    }
    .into();

    assert!(validate_workflow(&workflow).valid);
}

#[test]
fn validation_rejects_undeclared_variable_references() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = WorkflowNodeKind::Debug {
        value: ValueExpr::Ref {
            source: ValueSource::Variable {
                name: "missing".to_owned(),
            },
            pointer: String::new(),
        },
    }
    .into();

    let report = validate_workflow(&workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == ValidationIssueCode::UndeclaredVariable)
        .expect("undeclared variable reference should produce a stable issue");
    assert_eq!(issue.code.as_str(), "undeclared_variable");
    assert_eq!(issue.node_id.as_deref(), Some("log"));
    assert!(issue.message.contains("'missing' 未声明"));
}

#[test]
fn validation_rejects_undeclared_variable_assignments() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = NodeEnvelope::new(
        "argus.variable.set",
        1,
        json!({
            "assignments": [
                { "name": "missing", "value": { "type": "literal", "value": 1 } }
            ]
        }),
    );

    let report = validate_workflow(&workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == ValidationIssueCode::UndeclaredVariable)
        .expect("undeclared variable assignment should produce a stable issue");
    assert_eq!(issue.node_id.as_deref(), Some("log"));
    assert!(issue.message.contains("'missing' 未声明"));
}

#[test]
fn validation_accepts_declared_variable_references_and_assignments() {
    let mut workflow = demo_workflow(1);
    workflow.variables = json!({ "state": 1 });
    workflow.nodes[1].definition = NodeEnvelope::new(
        "argus.variable.set",
        1,
        json!({
            "assignments": [{
                "name": "state",
                "value": ValueExpr::Ref {
                    source: ValueSource::Variable {
                        name: "state".to_owned(),
                    },
                    pointer: String::new(),
                }
            }]
        }),
    );

    assert!(validate_workflow(&workflow).valid);
}

#[test]
fn validation_compiles_expressions_during_prepare() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1].definition = WorkflowNodeKind::Debug {
        value: ValueExpr::Expression {
            source: "input.[broken".to_owned(),
        },
    }
    .into();

    let report = validate_workflow(&workflow);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == ValidationIssueCode::InvalidExpression)
        .expect("invalid Rhai syntax should fail workflow preparation");
    assert_eq!(issue.node_id.as_deref(), Some("log"));
    assert!(issue.message.contains("表达式编译失败"));
}

#[test]
fn validation_rejects_empty_output_and_variable_assignment_names() {
    let mut workflow = demo_workflow(1);
    workflow.nodes[1]
        .output_bindings
        .insert("  ".to_owned(), ValueExpr::Literal { value: json!(1) });
    assert_has_issue(&workflow, ValidationIssueCode::InvalidOutputBinding);

    workflow.nodes[1].output_bindings.clear();
    workflow.nodes[1].definition = NodeEnvelope::new(
        "argus.variable.set",
        1,
        json!({
            "assignments": [
                { "name": "", "value": { "type": "literal", "value": 1 } }
            ]
        }),
    );
    assert_has_issue(&workflow, ValidationIssueCode::InvalidVariableAssignment);
}

#[test]
fn validation_requires_application_launch_permission_only_for_launching_policies() {
    let mut workflow = demo_workflow(1);
    let mut spec = test_application_spec();
    workflow.nodes[1].definition = WorkflowNodeKind::Application { spec: spec.clone() }.into();
    assert_has_issue(&workflow, ValidationIssueCode::ApplicationPermissionDenied);

    spec.acquire_policy = AcquirePolicy::AttachOnly;
    workflow.nodes[1].definition = WorkflowNodeKind::Application { spec }.into();
    assert!(validate_workflow(&workflow).valid);
}

/// 断言工作流包含指定稳定问题码。
fn assert_has_issue(workflow: &WorkflowDefinition, code: ValidationIssueCode) {
    let report = validate_workflow(workflow);
    assert!(report.issues.iter().any(|issue| issue.code == code));
}
