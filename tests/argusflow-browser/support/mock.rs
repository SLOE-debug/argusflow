//! 本地 CDP WebSocket 服务端替身。
use crate::{BrowserConfig, Page, cdp::Connection};
use argusflow_core::{Operation, OperationOptions};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

pub(super) struct Mock {
    pub(super) connection: Connection,
    pub(super) socket: WebSocketStream<TcpStream>,
}
impl Mock {
    pub(super) async fn new(capacity: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "ws://{}/devtools/browser/mock",
            listener.local_addr().unwrap()
        );
        let operation = Operation::new(OperationOptions::default());
        let config = BrowserConfig {
            max_in_flight: capacity,
            ..Default::default()
        };
        let (connection, socket) =
            tokio::join!(Connection::connect(&url, config, &operation), async {
                let (stream, _) = listener.accept().await.unwrap();
                accept_async(stream).await.unwrap()
            });
        Self {
            connection: connection.unwrap(),
            socket,
        }
    }
    pub(super) async fn next(&mut self) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(2), self.socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        serde_json::from_str(frame.to_text().unwrap()).unwrap()
    }
    pub(super) async fn send(&mut self, value: Value) {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .await
            .unwrap();
    }
    pub(super) async fn respond(&mut self, request: &Value, value: Value) {
        self.send(json!({"id":request["id"],"result":value})).await;
    }
    pub(super) async fn attach(&mut self) -> Page {
        let connection = self.connection.clone();
        let task = tokio::spawn(async move {
            Page::attach(
                connection,
                "target-1",
                &Operation::new(OperationOptions::default()),
            )
            .await
        });
        let request = self.next().await;
        assert_eq!(request["method"], "Target.attachToTarget");
        assert_eq!(request["params"]["flatten"], true);
        self.respond(&request, json!({"sessionId":"session-1"}))
            .await;
        for _ in 0..4 {
            let request = self.next().await;
            self.respond(&request, json!({})).await;
        }
        task.await.unwrap().unwrap()
    }
}
pub(super) fn options(ms: u64) -> OperationOptions {
    OperationOptions::new(Duration::from_millis(ms)).unwrap()
}
