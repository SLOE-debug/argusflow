use super::support::*;
use argusflow_runtime::*;
use argusflow_workflow::*;

fn timed_wait(timeout: Expr) -> Workflow {
    let mut wait = node(
        "wait",
        Action::Wait {
            milliseconds: Expr::int(30),
        },
    );
    wait.timeout_ms = Some(timeout);
    workflow(vec![scope("root", vec![wait], vec![])], Fields::new())
}

#[tokio::test]
async fn timeout_uses_each_runs_frozen_input_and_rejects_invalid_values() {
    let mut flow = timed_wait(Expr::Input {
        name: "等待时限".into(),
    });
    flow.inputs.insert("等待时限".into(), ValueType::Int);
    let plan = prepare(flow, &NodeRegistry::new()).unwrap();
    for (value, expected) in [
        (1, RunStatus::TimedOut),
        (1000, RunStatus::Completed),
        (0, RunStatus::Failed),
        (-1, RunStatus::Failed),
        (86_400_001, RunStatus::Failed),
    ] {
        let result = WorkflowEngine::new()
            .start(
                plan.clone(),
                RunInputs {
                    values: [("等待时限".into(), Value::Int(value))].into(),
                    ..Default::default()
                },
                RunOptions::default(),
            )
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            result.status, expected,
            "timeout={value}: {:?}",
            result.error
        );
    }
}

#[test]
fn timeout_cannot_read_unknown_input_wrong_type_or_current_nodes_output() {
    for expression in [
        Expr::text("10"),
        Expr::Input {
            name: "missing".into(),
        },
        Expr::NodeOutput {
            node: "wait".into(),
            output: "value".into(),
        },
    ] {
        let errors = prepare(timed_wait(expression), &NodeRegistry::new())
            .err()
            .unwrap();
        assert_eq!(errors[0].node.as_deref(), Some("wait"));
    }
}

#[tokio::test]
async fn prior_wait_does_not_consume_next_nodes_timeout() {
    let mut flow = timed_wait(Expr::int(1000));
    let wait = flow.scopes[0].nodes.remove(0);
    flow.scopes[0] = Scope::linear(
        "root",
        vec![
            node(
                "before",
                Action::Wait {
                    milliseconds: Expr::int(50),
                },
            ),
            wait,
        ],
    );
    assert_eq!(run(flow).await.status, RunStatus::Completed);
}
