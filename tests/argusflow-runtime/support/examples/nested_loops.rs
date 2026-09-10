//! 三次外循环各执行两次内循环，外层全局 total 最终为六。
mod common;
use argusflow_workflow::*;
use common::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = scope(
        "root",
        vec![
            declare("total", "total", Expr::int(0)),
            declare("outer", "outer", Expr::int(0)),
            Node::new(
                "outer-loop",
                Action::While {
                    condition: Expr::binary(BinaryOp::Less, Expr::var("outer"), Expr::int(3)),
                    body: "outer-body".into(),
                    max_iterations: None,
                },
            ),
        ],
        vec![("total", Expr::var("total"))],
    );
    let outer = scope(
        "outer-body",
        vec![
            declare("inner", "inner", Expr::int(0)),
            Node::new(
                "inner-loop",
                Action::While {
                    condition: Expr::binary(BinaryOp::Less, Expr::var("inner"), Expr::int(2)),
                    body: "inner-body".into(),
                    max_iterations: None,
                },
            ),
            increment("next-outer", "outer", Expr::int(1)),
        ],
        vec![],
    );
    let inner = scope(
        "inner-body",
        vec![
            increment("add-total", "total", Expr::int(1)),
            increment("next-inner", "inner", Expr::int(1)),
        ],
        vec![],
    );
    execute(workflow(
        "嵌套循环与全局累计",
        vec![root, outer, inner],
        [("total".into(), ValueType::Int)].into(),
    ))
    .await
}
