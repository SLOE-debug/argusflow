use super::*;
use argusflow_core::{FailureKind, Key};

#[tokio::test]
async fn ambiguous_css_and_document_replacement_invalidate_handles() {
    let mut mock = Mock::new(8).await;
    let page = mock.attach().await;
    let query_page = page.clone();
    let task = tokio::spawn(async move { query_page.find_unique("button", options(1000)).await });
    let request = mock.next().await;
    mock.respond(&request, json!({"root":{"nodeId":1}})).await;
    let request = mock.next().await;
    mock.respond(&request, json!({"nodeIds":[2,3]})).await;
    assert_eq!(
        task.await.unwrap().err().unwrap().kind(),
        FailureKind::Ambiguous
    );
    let query_page = page.clone();
    let task = tokio::spawn(async move { query_page.find_unique("#one", options(1000)).await });
    let request = mock.next().await;
    // 同一文档复用根节点，重复 getDocument 会使真实浏览器中的旧 nodeId 失效。
    assert_eq!(request["method"], "DOM.querySelectorAll");
    assert_eq!(request["params"]["nodeId"], 1);
    mock.respond(&request, json!({"nodeIds":[2]})).await;
    let element = task.await.unwrap().unwrap();
    mock.send(json!({"method":"DOM.documentUpdated","sessionId":"session-1","params":{}}))
        .await;
    // 通过后续响应建立事件已处理的先后关系。
    let query_page = page.clone();
    let sync = tokio::spawn(async move { query_page.evaluate("1", options(1000)).await });
    let request = mock.next().await;
    mock.respond(&request, json!({"result":{"value":1}})).await;
    sync.await.unwrap().unwrap();
    assert_eq!(
        element.read(options(1000)).await.err().unwrap().kind(),
        FailureKind::StaleHandle
    );
    let task = tokio::spawn(async move { page.find_unique("#new", options(1000)).await });
    let request = mock.next().await;
    assert_eq!(request["method"], "DOM.getDocument");
    mock.respond(&request, json!({"root":{"nodeId":10}})).await;
    let request = mock.next().await;
    assert_eq!(request["method"], "DOM.querySelectorAll");
    assert_eq!(request["params"]["nodeId"], 10);
    mock.respond(&request, json!({"nodeIds":[11]})).await;
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn session_destruction_clears_pending_requests() {
    let mut mock = Mock::new(2).await;
    let page = mock.attach().await;
    let task =
        tokio::spawn(async move { page.evaluate("new Promise(()=>{})", options(1000)).await });
    mock.next().await;
    mock.send(json!({"method":"Target.detachedFromTarget","params":{"sessionId":"session-1"}}))
        .await;
    assert_eq!(
        task.await.unwrap().unwrap_err().kind(),
        FailureKind::StaleHandle
    );
    assert!(
        mock.connection
            .inner
            .health
            .pages
            .lock()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn cancelled_keydown_is_released_and_input_stays_exclusive() {
    let mut mock = Mock::new(1).await;
    let page = mock.attach().await;
    let input_page = page.clone();
    let task = tokio::spawn(async move {
        input_page
            .press(&[Key::Control, Key::Letter('a')], options(1000))
            .await
    });
    let down = mock.next().await;
    assert_eq!(down["params"]["type"], "rawKeyDown");
    assert_eq!(
        page.insert_text("second", options(1000))
            .await
            .unwrap_err()
            .kind(),
        FailureKind::Busy
    );
    task.abort();
    let _ = task.await;
    let release = mock.next().await;
    assert_eq!(release["params"]["type"], "keyUp");
    assert_eq!(release["params"]["key"], "Control");
    mock.respond(&release, json!({})).await;
    // 已取消的 down 响应不会触发下一次字母按下。
    mock.respond(&down, json!({})).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let task = tokio::spawn(async move { page.insert_text("next", options(1000)).await });
    let request = mock.next().await;
    assert_eq!(request["method"], "Input.insertText");
    mock.respond(&request, json!({})).await;
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn remote_object_cleanup_uses_known_group_when_resolve_response_is_lost() {
    let mut mock = Mock::new(2).await;
    let page = mock.attach().await;
    let task = tokio::spawn(async move {
        page.find_unique("button", options(1000))
            .await
            .map(|element| (page, element))
    });
    let request = mock.next().await;
    mock.respond(&request, json!({"root":{"nodeId":1}})).await;
    let request = mock.next().await;
    mock.respond(&request, json!({"nodeIds":[2]})).await;
    let (_page, element) = task.await.unwrap().unwrap();
    let task = tokio::spawn(async move { element.read(options(1000)).await });
    let resolve = mock.next().await;
    assert_eq!(resolve["method"], "DOM.resolveNode");
    task.abort();
    let _ = task.await;
    let release = mock.next().await;
    assert_eq!(release["method"], "Runtime.releaseObjectGroup");
    assert_eq!(
        release["params"]["objectGroup"],
        resolve["params"]["objectGroup"]
    );
    mock.respond(&release, json!({})).await;
}
