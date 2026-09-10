//! 文档/Shadow 嵌套、空匹配与跨进程输入路由的协议契约。
use super::super::{Mock, json, options};
use super::fixture::*;
use argusflow_aql::{Bindings, compile};
use argusflow_core::Operation;

#[tokio::test]
async fn document_frame_shadow_nesting_keeps_boundaries_explicit() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let mut panel = element(4, "custom-panel");
    panel["shadowRoots"] = json!([{"nodeId":5,"backendNodeId":5,"nodeType":11,"shadowRootType":"open","children":[element(6,"input")]}]);
    let mut host = element(2, "iframe");
    host["frameId"] = json!("child");
    host["contentDocument"] = document(3, vec![panel]);
    let query = compile(
        "document() >> frame(element(dom.tag = \"iframe\")) >> shadow(pane()) >> textbox()",
    )
    .unwrap()
    .bind(&Bindings::new())
    .unwrap();
    let task =
        tokio::spawn(async move { page.query_aql(&query, &Operation::new(options(2000))).await });
    root(&mut mock, document(1, vec![host])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "Iframe")],
        vec![facts("/0", "iframe")],
        "session-1",
    )
    .await;
    snapshot(
        &mut mock,
        3,
        "child",
        vec![(4, "generic")],
        vec![facts("/0", "custom-panel")],
        "session-1",
    )
    .await;
    scope_snapshot(
        &mut mock,
        5,
        "child",
        vec![(6, "textbox")],
        vec![facts("/0", "input")],
        "session-1",
    )
    .await;
    let found = task.await.unwrap().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].frame_id(), "child");
}

#[tokio::test]
async fn ordinary_descendants_never_enter_an_iframe() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let mut host = element(2, "iframe");
    host["frameId"] = json!("child");
    host["contentDocument"] = document(3, vec![element(4, "button")]);
    let query = compile("document() >> button()")
        .unwrap()
        .bind(&Bindings::new())
        .unwrap();
    let task =
        tokio::spawn(async move { page.query_aql(&query, &Operation::new(options(2000))).await });
    root(&mut mock, document(1, vec![host])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "Iframe")],
        vec![facts("/0", "iframe")],
        "session-1",
    )
    .await;
    assert!(task.await.unwrap().unwrap().is_empty());
}
