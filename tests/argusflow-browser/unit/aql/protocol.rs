//! 查询语义依赖真实协议边界，替身不执行网页或读取用户页面。
use super::super::{Mock, json, options};
use super::fixture::*;
use argusflow_aql::{Bindings, compile};
use argusflow_core::{Effect, FailureKind, Operation};
fn query(source: &str) -> argusflow_aql::BoundQuery {
    compile(source).unwrap().bind(&Bindings::new()).unwrap()
}

#[tokio::test]
async fn ax_names_filter_in_rust_and_input_preserves_text() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let task = tokio::spawn(async move {
        page.query_aql(
            &query("textbox(name matches /保存/)"),
            &Operation::new(options(2000)),
        )
        .await
    });
    root(&mut mock, document(1, vec![element(2, "input")])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "textbox")],
        vec![facts("/0", "input")],
        "session-1",
    )
    .await;
    let targets = task.await.unwrap().unwrap();
    assert_eq!(targets.len(), 1);
    let target = targets[0].clone();
    let task = tokio::spawn(async move {
        target
            .type_text_aql("追加", &Operation::new(options(2000)))
            .await
    });
    action(&mut mock, "focus", json!(true)).await;
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Input.insertText");
    assert_eq!(request["params"]["text"], "追加");
    mock.respond(&request, json!({})).await;
    task.await.unwrap().unwrap();
}
#[tokio::test]
async fn shadow_is_loaded_only_after_unique_host_and_returns_inner_target() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let mut host = element(2, "custom-panel");
    host["shadowRoots"] = json!([{"nodeId":3,"backendNodeId":3,"nodeType":11,"shadowRootType":"open","children":[element(4,"button")]}]);
    let task = tokio::spawn(async move {
        page.query_aql(
            &query("shadow(pane()) >> button()"),
            &Operation::new(options(2000)),
        )
        .await
    });
    root(&mut mock, document(1, vec![host])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "generic"), (4, "button")],
        vec![facts("/0", "custom-panel")],
        "session-1",
    )
    .await;
    scope_snapshot(
        &mut mock,
        3,
        "main",
        vec![(2, "generic"), (4, "button")],
        vec![facts("/0", "button")],
        "session-1",
    )
    .await;
    let targets = task.await.unwrap().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].role(), argusflow_aql::Role::Button);
}
#[tokio::test]
async fn same_process_iframe_uses_its_frame_ax_and_parent_click_coordinates() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let mut host = element(2, "iframe");
    host["frameId"] = json!("child");
    host["contentDocument"] = document(3, vec![element(4, "button")]);
    let task = tokio::spawn(async move {
        page.query_aql(
            &query("frame(element(dom.tag = \"iframe\")) >> button()"),
            &Operation::new(options(2000)),
        )
        .await
    });
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
        vec![(4, "button")],
        vec![facts("/0", "button")],
        "session-1",
    )
    .await;
    let target = task.await.unwrap().unwrap().remove(0);
    assert_eq!(target.frame_id(), "child");
    let task = tokio::spawn(async move { target.click_aql(&Operation::new(options(2000))).await });
    for id in [2, 4] {
        let request = next(&mut mock).await;
        assert_eq!(request["method"], "DOM.scrollIntoViewIfNeeded");
        assert_eq!(request["params"]["backendNodeId"], id);
        mock.respond(&request, json!({})).await;
    }
    action(&mut mock, "point", json!([30, 30])).await;
    action(&mut mock, "frame", json!({"left":100,"top":200,"width":204,"height":104,"border_left":2,"border_top":2,"layout_width":204,"layout_height":104,"supported":true})).await;
    action(&mut mock, "hit", json!(true)).await;
    for kind in ["mouseMoved", "mousePressed", "mouseReleased"] {
        let request = next(&mut mock).await;
        assert_eq!(request["method"], "Input.dispatchMouseEvent");
        assert_eq!(request["params"]["type"], kind);
        assert_eq!(request["params"]["x"], 132.0);
        assert_eq!(request["params"]["y"], 232.0);
        mock.respond(&request, json!({})).await;
    }
    task.await.unwrap().unwrap();
}
#[tokio::test]
async fn cross_process_frame_attaches_own_session_and_releases_it() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let mut host = element(2, "iframe");
    host["frameId"] = json!("child");
    let task = tokio::spawn(async move {
        page.query_aql(
            &query("frame(element(dom.tag = \"iframe\")) >> textbox()"),
            &Operation::new(options(2000)),
        )
        .await
    });
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
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Target.getTargets");
    mock.respond(
        &request,
        json!({"targetInfos":[{"targetId":"child","type":"iframe"}]}),
    )
    .await;
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Target.attachToTarget");
    assert_eq!(request["params"]["targetId"], "child");
    mock.respond(&request, json!({"sessionId":"child-session"}))
        .await;
    for _ in 0..4 {
        let request = next(&mut mock).await;
        assert_eq!(request["sessionId"], "child-session");
        mock.respond(&request, json!({})).await;
    }
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Page.getFrameTree");
    mock.respond(&request, json!({"frameTree":{"frame":{"id":"child"}}}))
        .await;
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "DOM.getDocument");
    mock.respond(
        &request,
        json!({"root":document(10,vec![element(11,"input")])}),
    )
    .await;
    snapshot(
        &mut mock,
        10,
        "child",
        vec![(11, "textbox")],
        vec![facts("/0", "input")],
        "child-session",
    )
    .await;
    let results = task.await.unwrap().unwrap();
    assert_eq!(results[0].frame_id(), "child");
    let target = results[0].clone();
    let typing = tokio::spawn(async move {
        target
            .type_text_aql("追加", &Operation::new(options(2000)))
            .await
    });
    action(&mut mock, "focus", json!(true)).await;
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Input.insertText");
    assert_eq!(request["sessionId"], "child-session");
    mock.respond(&request, json!({})).await;
    typing.await.unwrap().unwrap();
    drop(results);
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Target.detachFromTarget");
    assert_eq!(request["params"]["sessionId"], "child-session");
    mock.respond(&request, json!({})).await;
}
#[tokio::test]
async fn child_navigation_invalidates_parent_proof_and_no_input_is_sent() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let query_page = page.clone();
    let task = tokio::spawn(async move {
        query_page
            .query_aql(&query("button()"), &Operation::new(options(2000)))
            .await
    });
    root(&mut mock, document(1, vec![element(2, "button")])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "button")],
        vec![facts("/0", "button")],
        "session-1",
    )
    .await;
    let target = task.await.unwrap().unwrap().remove(0);
    mock.send(json!({"method":"Page.frameNavigated","sessionId":"session-1","params":{"frame":{"id":"child","parentId":"main"}}})).await;
    let sync = tokio::spawn(async move { page.evaluate("1", options(1000)).await });
    let request = next(&mut mock).await;
    mock.respond(&request, json!({"result":{"value":1}})).await;
    sync.await.unwrap().unwrap();
    assert_eq!(
        target
            .click_aql(&Operation::new(options(1000)))
            .await
            .unwrap_err()
            .kind(),
        FailureKind::StaleHandle
    );
}
#[tokio::test]
async fn input_failure_keeps_effect_and_does_not_retry() {
    let mut mock = Mock::new(16).await;
    let page = mock.attach().await;
    let task = tokio::spawn(async move {
        page.query_aql(&query("textbox()"), &Operation::new(options(2000)))
            .await
    });
    root(&mut mock, document(1, vec![element(2, "input")])).await;
    snapshot(
        &mut mock,
        1,
        "main",
        vec![(2, "textbox")],
        vec![facts("/0", "input")],
        "session-1",
    )
    .await;
    let target = task.await.unwrap().unwrap().remove(0);
    let task = tokio::spawn(async move {
        target
            .type_text_aql("追加", &Operation::new(options(300)))
            .await
    });
    action(&mut mock, "focus", json!(true)).await;
    let request = next(&mut mock).await;
    assert_eq!(request["method"], "Input.insertText");
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.kind(), FailureKind::Timeout);
    assert_eq!(error.effect(), Effect::Unconfirmed);
}
