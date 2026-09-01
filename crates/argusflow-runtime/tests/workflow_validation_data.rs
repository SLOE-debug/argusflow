//! schema v10 工作流输入、变量、表达式和输出映射校验契约。

mod workflow_fixture;

use argusflow_core::{
    AcquirePolicy, NodeEnvelope, ValueExpr, ValueSource, WorkflowDefinition,
    WorkflowInputDefinition, WorkflowInputType,
};
use argusflow_runtime::{ValidationIssueCode, validate_workflow};
use serde_json::json;

use workflow_fixture::{WorkflowNodeKind, demo_workflow, test_application_spec};

#[test]
fn validation_uses_input_declarations_instead_of_persisted_variables() {
    let mut workflow = demo_workflow(1);
    workflow.inputs = vec![WorkflowInputDefinition {
        key: "secret".to_owned(),
        value_type: WorkflowInputType::Text,
    }];
    workflow.variables = json!({ "secret": 42 });
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Debug {
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
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Debug {
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
    workflow.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
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
    workflow.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
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
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Debug {
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
    workflow.graph.scopes[0].nodes[1]
        .output_bindings
        .insert("  ".to_owned(), ValueExpr::Literal { value: json!(1) });
    assert_has_issue(&workflow, ValidationIssueCode::InvalidOutputBinding);

    workflow.graph.scopes[0].nodes[1].output_bindings.clear();
    workflow.graph.scopes[0].nodes[1].definition = NodeEnvelope::new(
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
    workflow.graph.scopes[0].nodes[1].definition =
        WorkflowNodeKind::Application { spec: spec.clone() }.into();
    assert_has_issue(&workflow, ValidationIssueCode::ApplicationPermissionDenied);

    spec.acquire_policy = AcquirePolicy::AttachOnly;
    workflow.graph.scopes[0].nodes[1].definition = WorkflowNodeKind::Application { spec }.into();
    assert!(validate_workflow(&workflow).valid);
}

/// 断言工作流包含指定稳定问题码。
fn assert_has_issue(workflow: &WorkflowDefinition, code: ValidationIssueCode) {
    let report = validate_workflow(workflow);
    assert!(report.issues.iter().any(|issue| issue.code == code));
}
