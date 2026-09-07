use crate::cdp::{CdpConnection, CdpPageSession};
use argusflow_core::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::test]
async fn coordinate_hit_test_and_object_cleanup_use_only_the_attached_session() {
    // 本地协议 fixture，不启动或自动操作任何浏览器。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let commands = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = commands.clone();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(socket).await.unwrap();
        while let Some(Ok(Message::Text(message))) = socket.next().await {
            let request: Value = serde_json::from_str(&message).unwrap();
            let method = request["method"].as_str().unwrap();
            observed.lock().unwrap().push(request.clone());
            let result = match method {
                "Target.attachToTarget" => json!({ "sessionId": "attached-session" }),
                "Runtime.enable" | "Page.enable" | "Inspector.enable" => json!({}),
                "Runtime.evaluate" => json!({ "result": { "value": { "width": 800, "height": 600,
                    "dpr": 1.5, "scale": 1, "offset_x": 0, "offset_y": 0, "focused": true,
                    "visible": true, "url": "https://example.test/form" } } }),
                "DOM.getNodeForLocation" => {
                    assert_eq!(request["params"]["x"], 100);
                    assert_eq!(request["params"]["y"], 200);
                    json!({ "backendNodeId": 99, "frameId": "main" })
                }
                "DOM.resolveNode" => json!({ "object": { "objectId": "object-99" } }),
                "Runtime.callFunctionOn" => json!({ "result": { "value": {
                    "semantics": { "role": "text_box", "name": "Name", "test_id": "name-input" },
                    "ancestors": [], "bounds": { "x": 100, "y": 200, "width": 40, "height": 20 },
                    "editable": true, "sensitive": false, "replayable": true,
                } } }),
                "DOM.describeNode" => json!({ "node": { "backendNodeId": 99 } }),
                "Runtime.releaseObjectGroup" => {
                    assert!(
                        request["params"]["objectGroup"]
                            .as_str()
                            .unwrap()
                            .starts_with("argusflow-inspection-")
                    );
                    socket
                        .send(Message::Text(
                            json!({ "id": request["id"], "result": {} })
                                .to_string()
                                .into(),
                        ))
                        .await
                        .unwrap();
                    break;
                }
                other => panic!("unexpected mutating or discovery command: {other}"),
            };
            socket
                .send(Message::Text(
                    json!({ "id": request["id"], "result": result })
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
        }
    });
    let connection = CdpConnection::connect(&format!("ws://{address}"))
        .await
        .unwrap();
    let page = CdpPageSession::attach(connection, "managed-target".into())
        .await
        .unwrap();
    let context = InspectionContext {
        window: WindowIdentity {
            handle: 7,
            process_id: 8,
        },
        executable_path: None,
        title: "Page".into(),
        class_name: "Chrome_WidgetWin_1".into(),
        dpi: 144,
        has_keyboard_focus: true,
        bounds: InspectionRect {
            x: -1800.0,
            y: -500.0,
            width: 1200.0,
            height: 1000.0,
        },
        browser_viewport: Some(InspectionRect {
            x: -1800.0,
            y: -400.0,
            width: 1200.0,
            height: 900.0,
        }),
    };
    let resource = ResourceId::new();
    let entity = super::inspector::inspect_attached(
        resource,
        page,
        &context,
        InspectionProbe::Point(ScreenPoint { x: -1650, y: -100 }),
    )
    .await
    .unwrap();
    assert_eq!(entity.browser_session, Some(resource));
    assert_eq!(entity.bounds.x, -1650.0);
    assert_eq!(entity.semantics.test_id.as_deref(), Some("name-input"));
    tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert!(
        commands
            .lock()
            .unwrap()
            .iter()
            .skip(1)
            .all(|command| command["sessionId"] == "attached-session")
    );
}

#[tokio::test]
async fn unknown_browser_never_discovers_or_attaches_new_pages() {
    let runtime = crate::CdpRuntime::new();
    let context = InspectionContext {
        window: WindowIdentity {
            handle: 7,
            process_id: 8,
        },
        executable_path: None,
        title: "Page".into(),
        class_name: "Chrome_WidgetWin_1".into(),
        dpi: 96,
        has_keyboard_focus: true,
        bounds: InspectionRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        browser_viewport: None,
    };
    assert_eq!(
        runtime.inspect(&context, InspectionProbe::Focus).await,
        Err(InspectionFailure::UnmanagedWindow)
    );
}
