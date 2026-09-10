//! 为每个请求明确提供 DOM、AX 和属性事实。
use super::super::{Mock, Value, json};

pub(super) fn element(id: i64, tag: &str) -> Value {
    json!({"nodeId":id,"backendNodeId":id,"nodeType":1,"localName":tag,"children":[],"shadowRoots":[]})
}
pub(super) fn document(id: i64, children: Vec<Value>) -> Value {
    json!({"nodeId":id,"backendNodeId":id,"nodeType":9,"localName":"","children":children})
}
pub(super) fn facts(path: &str, tag: &str) -> Value {
    json!({"path":path,"text":"保存","value":if tag == "input" {json!("已有")} else {Value::Null},
        "enabled":true,"visible":true,"focused":false,"checked":null,"selected":null,
        "attributes":{},"css":[],"bounds":[10,20,40,20]})
}
pub(super) async fn next(mock: &mut Mock) -> Value {
    loop {
        let request = mock.next().await;
        if request["method"] == "Runtime.releaseObjectGroup" {
            mock.respond(&request, json!({})).await;
        } else {
            return request;
        }
    }
}
pub(super) async fn root(mock: &mut Mock, doc: Value) {
    let request = next(mock).await;
    assert_eq!(request["method"], "Page.getFrameTree");
    mock.respond(&request, json!({"frameTree":{"frame":{"id":"main"}}}))
        .await;
    let request = next(mock).await;
    assert_eq!(request["method"], "DOM.getDocument");
    mock.respond(&request, json!({"root":doc})).await;
}
pub(super) async fn snapshot(
    mock: &mut Mock,
    root_id: i64,
    frame: &str,
    nodes: Vec<(i64, &str)>,
    facts: Vec<Value>,
    session: &str,
) {
    let mut nodes = nodes;
    let mut facts = facts;
    nodes.insert(0, (root_id, "RootWebArea"));
    facts.insert(0, self::facts("", ""));
    scope_snapshot(mock, root_id, frame, nodes, facts, session).await;
}
pub(super) async fn scope_snapshot(
    mock: &mut Mock,
    root_id: i64,
    frame: &str,
    nodes: Vec<(i64, &str)>,
    facts: Vec<Value>,
    session: &str,
) {
    let request = next(mock).await;
    assert_eq!(request["method"], "Accessibility.getFullAXTree");
    assert_eq!(request["sessionId"], session);
    assert_eq!(request["params"]["frameId"], frame);
    mock.respond(&request, json!({"nodes":nodes.into_iter().map(|(id,role)| json!({"backendDOMNodeId":id,"role":{"value":role},"name":{"value":"保存"}})).collect::<Vec<_>>()})).await;
    let request = next(mock).await;
    assert_eq!(request["method"], "DOM.resolveNode");
    assert_eq!(request["params"]["backendNodeId"], root_id);
    mock.respond(
        &request,
        json!({"object":{"objectId":format!("root-{root_id}")}}),
    )
    .await;
    let request = next(mock).await;
    assert_eq!(request["method"], "Runtime.callFunctionOn");
    mock.respond(&request, json!({"result":{"value":facts}}))
        .await;
}
pub(super) async fn action(mock: &mut Mock, action: &str, result: Value) {
    let request = next(mock).await;
    assert_eq!(request["method"], "DOM.resolveNode");
    mock.respond(&request, json!({"object":{"objectId":"target"}}))
        .await;
    let request = next(mock).await;
    assert_eq!(request["method"], "Runtime.callFunctionOn");
    assert_eq!(request["params"]["arguments"][0]["value"], action);
    mock.respond(&request, json!({"result":{"value":result}}))
        .await;
}
