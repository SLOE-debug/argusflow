use super::*;
use crate::{Browser, ConnectionState};

#[tokio::test]
async fn external_browser_drop_disconnects_even_when_page_survives() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "ws://{}/devtools/browser/mock",
        listener.local_addr().unwrap()
    );
    let (browser, socket) = tokio::join!(
        Browser::connect(&endpoint, BrowserConfig::default(), options(1000)),
        async {
            let (stream, _) = listener.accept().await.unwrap();
            accept_async(stream).await.unwrap()
        }
    );
    let browser = browser.unwrap();
    let mut socket = socket;
    let owner = browser.clone();
    let task = tokio::spawn(async move { owner.attach("target-1", options(1000)).await });
    for i in 0..5 {
        let message = socket.next().await.unwrap().unwrap();
        let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        let result = if i == 0 {
            json!({"sessionId":"session-1"})
        } else {
            json!({})
        };
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":result})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
    }
    let page = task.await.unwrap().unwrap();
    assert!(page.is_open());
    drop(browser);
    assert!(!page.is_open());
    assert_eq!(page.inner.connection.state(), ConnectionState::Disconnected);
    // 对端只看到断连，绝不能收到关闭外部浏览器的命令。
    let frame = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap();
    assert!(!matches!(frame, Some(Ok(Message::Text(_)))));
}

#[tokio::test]
async fn new_page_uses_connection_owned_context_and_disposes_it_on_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "ws://{}/devtools/browser/mock",
        listener.local_addr().unwrap()
    );
    let (browser, socket) = tokio::join!(
        Browser::connect(&endpoint, BrowserConfig::default(), options(1000)),
        async {
            let (stream, _) = listener.accept().await.unwrap();
            accept_async(stream).await.unwrap()
        }
    );
    let browser = browser.unwrap();
    let mut socket = socket;
    let owner = browser.clone();
    let task = tokio::spawn(async move { owner.new_page("about:blank", options(1000)).await });
    for i in 0..7 {
        let message = socket.next().await.unwrap().unwrap();
        let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        let result = match i {
            0 => {
                assert_eq!(request["method"], "Target.createBrowserContext");
                assert_eq!(request["params"]["disposeOnDetach"], true);
                json!({"browserContextId":"context-1"})
            }
            1 => {
                assert_eq!(request["params"]["browserContextId"], "context-1");
                json!({"targetId":"new-target"})
            }
            2 => json!({"sessionId":"session-1"}),
            _ => json!({}),
        };
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":result})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
    }
    let page = task.await.unwrap().unwrap();
    let task = tokio::spawn(async move { page.close(options(1000)).await });
    for method in ["Target.closeTarget", "Target.disposeBrowserContext"] {
        let message = socket.next().await.unwrap().unwrap();
        let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
        assert_eq!(request["method"], method);
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":{"success":true}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
    }
    task.await.unwrap().unwrap();
    browser.shutdown(options(1000)).await.unwrap();
}
