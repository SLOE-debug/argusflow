//! 完全本地的 CDP 协议替身；不会创建或控制浏览器。
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{accept_async, tungstenite::Message};

pub struct Server {
    pub endpoint: String,
    pub calls: Arc<Mutex<Vec<Value>>>,
    task: JoinHandle<()>,
}
impl Server {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "ws://{}/devtools/browser/test",
            listener.local_addr().unwrap()
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(socket).await.unwrap();
            while let Some(Ok(Message::Text(frame))) = socket.next().await {
                let request: Value = serde_json::from_str(&frame).unwrap();
                recorded.lock().unwrap().push(request.clone());
                let result = match request["method"].as_str().unwrap() {
                    "Target.getTargets" => {
                        json!({"targetInfos":[{"targetId":"external-page","type":"page","title":"fixture","url":"https://fixture.invalid/"}]})
                    }
                    "Target.attachToTarget" => {
                        json!({"sessionId":format!("session-{}", request["params"]["targetId"].as_str().unwrap())})
                    }
                    "Target.createBrowserContext" => json!({"browserContextId":"own-context"}),
                    "Target.createTarget" => json!({"targetId":"own-page"}),
                    "Page.navigate" => json!({"loaderId":"loader"}),
                    "Page.getFrameTree" => {
                        json!({"frameTree":{"frame":{"id":"main","loaderId":"loader"}}})
                    }
                    "Runtime.evaluate" => json!({"result":{"value":"complete"}}),
                    "DOM.getDocument" => {
                        json!({"root":{"nodeId":1,"backendNodeId":1,"nodeType":9,"localName":"","children":[{"nodeId":2,"backendNodeId":2,"nodeType":1,"localName":"button","children":[],"shadowRoots":[]}]}})
                    }
                    "Accessibility.getFullAXTree" => {
                        json!({"nodes":[{"backendDOMNodeId":1,"role":{"value":"RootWebArea"}},{"backendDOMNodeId":2,"role":{"value":"button"},"name":{"value":"Save"}}]})
                    }
                    "DOM.resolveNode" => json!({"object":{"objectId":"root"}}),
                    "Runtime.callFunctionOn" => {
                        json!({"result":{"value":[facts("", ""), facts("/0", "Save")]}})
                    }
                    "Runtime.enable"
                    | "Page.enable"
                    | "DOM.enable"
                    | "Inspector.enable"
                    | "Runtime.releaseObjectGroup"
                    | "Target.detachFromTarget"
                    | "Target.closeTarget"
                    | "Target.disposeBrowserContext" => json!({}),
                    method => panic!("unexpected CDP method {method}"),
                };
                if socket
                    .send(Message::Text(
                        json!({"id":request["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            endpoint,
            calls,
            task,
        }
    }
}
fn facts(path: &str, text: &str) -> Value {
    json!({"path":path,"text":text,"value":null,"enabled":true,"visible":true,"focused":false,"checked":null,"selected":null,"attributes":{},"css":[],"bounds":[10,20,40,30]})
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
