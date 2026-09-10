//! 子流程参数独立，每次调用更新根全局，返回值保持调用结束时的快照。
mod common;
use argusflow_workflow::*;
use common::*;

fn call(id: &str, amount: i64) -> Node {
    Node::new(
        id,
        Action::Call {
            subflow: "accumulate".into(),
            inputs: [("amount".into(), Expr::int(amount))].into(),
            resources: Default::default(),
        },
    )
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = scope(
        "root",
        vec![
            declare("total", "total", Expr::int(0)),
            call("first", 2),
            call("second", 5),
        ],
        vec![
            (
                "first",
                Expr::NodeOutput {
                    node: "first".into(),
                    output: "sum".into(),
                },
            ),
            (
                "second",
                Expr::NodeOutput {
                    node: "second".into(),
                    output: "sum".into(),
                },
            ),
        ],
    );
    let body = scope(
        "accumulate-body",
        vec![increment(
            "add",
            "total",
            Expr::Input {
                name: "amount".into(),
            },
        )],
        vec![("sum", Expr::var("total"))],
    );
    let mut flow = workflow(
        "参数与返回值",
        vec![root, body],
        [
            ("first".into(), ValueType::Int),
            ("second".into(), ValueType::Int),
        ]
        .into(),
    );
    flow.subflows.insert(
        "accumulate".into(),
        Subflow {
            scope: "accumulate-body".into(),
            inputs: [("amount".into(), ValueType::Int)].into(),
            resources: Default::default(),
            outputs: [("sum".into(), ValueType::Int)].into(),
        },
    );
    execute(flow).await
}
