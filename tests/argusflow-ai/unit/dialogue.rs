use super::super::{service, validation};
use crate::{
    AiConfig, AiError, CancellationToken, Evidence,
    transport::{Response, Transport},
};
use argusflow_recorder::*;
use argusflow_runtime::NodeRegistry;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
fn evidence() -> Arc<Evidence> {
    let session = Session {
        id: "test".into(),
        format: 2,
        created_ms: 0,
        qpc_origin: 0,
        qpc_frequency: 1000,
        capture_session: None,
        policy: "test".into(),
    };
    let raw:Record=serde_json::from_value(json!({"id":1,"written_qpc":1,"data":{"Interaction":{"raw":[],"kind":"Click","from_qpc":1,"through_qpc":2,"window":{"handle":1,"pid":1,"epoch":1},"basis":"observed","related":null}}})).unwrap();
    Arc::new(Evidence {
        directory: Default::default(),
        session,
        records: [(1, raw)].into(),
    })
}
fn unresolved() -> Value {
    json!({"workflow":null,"analysis":{"summary":"目标缺失","node_evidence":[],"unresolved":[{"evidence_ids":["1"],"reason":"缺少目标证据"}],"required_bindings":[],"replay_ready":false}})
}
struct Script {
    replies: Mutex<Vec<Value>>,
    seen: Arc<Mutex<Vec<Vec<Value>>>>,
}
impl Transport for Script {
    async fn complete(
        &self,
        messages: &[Value],
        _: Vec<Value>,
        _: &CancellationToken,
    ) -> crate::Result<Response> {
        self.seen.lock().unwrap().push(messages.to_vec());
        Ok(serde_json::from_value(self.replies.lock().unwrap().remove(0)).unwrap())
    }
}
fn response(content: Value) -> Value {
    json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":content.to_string()}}],"usage":{"total_tokens":1}})
}
#[tokio::test]
async fn tools_and_compile_feedback_share_the_same_conversation() {
    let seen = Arc::new(Mutex::new(vec![]));
    let mut bad = unresolved();
    bad["analysis"]["unresolved"][0]["evidence_ids"] = json!(["999"]);
    let client = Script {
        seen: seen.clone(),
        replies: Mutex::new(vec![
            json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","tool_calls":[{"id":"call1","type":"function","function":{"name":"inspect_evidence","arguments":"{\"ids\":[\"1\"]}"}}]}}]}),
            response(bad),
            response(unresolved()),
        ]),
    };
    let config = AiConfig {
        vision: false,
        ..Default::default()
    };
    let result = service::run(
        config,
        client,
        evidence(),
        &NodeRegistry::new(),
        CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(result.metrics.rounds, 3);
    assert_eq!(result.metrics.tool_calls, 1);
    let history = seen.lock().unwrap();
    assert!(
        history[1]
            .iter()
            .any(|m| m["role"] == "tool" && m["tool_call_id"] == "call1")
    );
    assert!(history[2].iter().any(|m| {
        m["content"]
            .as_str()
            .is_some_and(|s| s.contains("校验失败"))
    }));
}
#[tokio::test]
async fn cancellation_prevents_network_and_tools() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    let client = Script {
        replies: Mutex::new(vec![]),
        seen: Default::default(),
    };
    assert!(matches!(
        service::run(
            AiConfig {
                vision: false,
                ..Default::default()
            },
            client,
            evidence(),
            &NodeRegistry::new(),
            cancel,
            |_| {}
        )
        .await,
        Err(AiError::Cancelled)
    ));
}
#[test]
fn invented_evidence_and_claimed_replay_are_rejected() {
    let e = evidence();
    let registry = NodeRegistry::new();
    let mut result = unresolved();
    result["analysis"]["replay_ready"] = json!(true);
    assert!(validation::validate(&serde_json::from_value(result).unwrap(), &e, &registry).is_err());
    let mut result = unresolved();
    result["analysis"]["unresolved"] = json!([]);
    assert!(validation::validate(&serde_json::from_value(result).unwrap(), &e, &registry).is_err());
}

#[test]
fn workflow_must_pass_actual_registry_and_cover_every_node() {
    let mut value = unresolved();
    value["analysis"]["unresolved"] = json!([]);
    value["analysis"]["node_evidence"] =
        json!([{"node_id":"n1","evidence_ids":["1"],"outcome":"request_only","uncertainty":null}]);
    let timeout =
        json!({"kind":"literal","value_type":{"type":"int"},"value":{"type":"int","value":1000}});
    value["workflow"] = json!({"name":"候选","inputs":{},"outputs":{},"resources":{},"root":"root","subflows":{},"scopes":[{"id":"root","nodes":[{"id":"n1","timeout_ms":timeout,"output_bindings":{},"action":{"kind":"task","task":{"type_id":"invented.task","version":1,"config":{},"inputs":{},"resources":{},"resource_outputs":{},"retry":null}}}],"edges":[{"id":"e1","source":{"kind":"start"},"target":{"kind":"node","node":"n1"}},{"id":"e2","source":{"kind":"node","node":"n1"},"target":{"kind":"end"}}],"outputs":{}}]});
    let candidate = serde_json::from_value(value.clone()).unwrap();
    let error = validation::validate(&candidate, &evidence(), &NodeRegistry::new()).unwrap_err();
    assert!(error.to_string().contains("编译失败"));
    value["workflow"]["scopes"][0]["nodes"][0]["action"] =
        json!({"kind":"wait","milliseconds":timeout});
    let candidate = serde_json::from_value(value.clone()).unwrap();
    validation::validate(&candidate, &evidence(), &NodeRegistry::new()).unwrap();
    value["analysis"]["node_evidence"][0]["node_id"] = json!("不存在");
    assert!(
        validation::validate(
            &serde_json::from_value(value).unwrap(),
            &evidence(),
            &NodeRegistry::new()
        )
        .is_err()
    );
}
