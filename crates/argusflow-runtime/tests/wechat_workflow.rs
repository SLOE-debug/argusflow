use argusflow_core::WorkflowDefinition;
use argusflow_runtime::validate_workflow;
use serde_json::{Value, json};

#[test]
fn generalized_runtime_accepts_expanded_wechat_search_send_and_verify_workflow() {
    let workflow: WorkflowDefinition =
        serde_json::from_value(workflow_definition()).expect("微信示例工作流应符合核心契约");

    let report = validate_workflow(&workflow);
    assert!(
        report.valid,
        "微信示例应能被通用 Runtime 直接校验：{:#?}",
        report.issues
    );
}

/// 构造与 Studio 默认画布相同职责和协议版本的完整工作流。
fn workflow_definition() -> Value {
    let application_id = "open_wechat";
    let application_scope = json!({
        "type": "application",
        "resource": { "producer_node_id": application_id, "output_name": "session" }
    });
    let contact_input = input("联系人");
    let message_input = input("消息内容");
    let search_ready = "exists(text(name contains \"网络结果\"))";
    let conversation_ready = "exists(nearest(anchor = text(name = \"搜索\"), target = text(name = $contact_name), direction = any, index = 2))";
    let message_sent = "all_of(exists(nearest(anchor = text(name = \"搜索\"), target = text(name = $contact_name), direction = any, index = 2)), exists(nearest(anchor = viewport_edge(side = bottom), target = text(name = $message), direction = any, index = 1)), not(exists(text(name contains \"重新发送\"))))";

    json!({
        "schema_version": 10,
        "id": "22222222-2222-4222-8222-222222222222",
        "name": "微信：搜索联系人并发送消息",
        "inputs": [
            { "key": "联系人", "value_type": "text" },
            { "key": "消息内容", "value_type": "text" }
        ],
        "variables": {},
        "permissions": { "allow": ["process.application.launch"] },
        "graph": {
          "root_scope_id": "root",
          "scopes": [{
            "id": "root",
            "parent": null,
            "boundary": { "type": "workflow", "entry_node_id": "start" },
            "nodes": [
            node("start", "argus.start", 1, json!({})),
            node(application_id, "argus.application", 1, json!({
                "spec": {
                    "executable_path": "C:\\Program Files\\Tencent\\Weixin\\Weixin.exe",
                    "arguments": [],
                    "window_title": { "type": "equal", "value": "微信" },
                    "acquire_policy": "attach_or_start",
                    "launch_timeout_ms": 15000,
                    "cleanup_policy": "leave_running",
                    "activation_policy": "required"
                }
            })),
            ui_node("open_search", key_operation(&application_scope, "f", &["control"])),
            loop_node("wait_for_search", "wait_for_search_body"),
            fail_node(
                "search_not_ready",
                "wechat_search_not_ready",
                "未能打开微信搜索。请确认微信窗口可以正常操作后重试。",
            ),
            ui_node("select_search_text", key_operation(&application_scope, "a", &["control"])),
            ui_node("type_contact", text_operation(&application_scope, contact_input.clone())),
            node("select_contact", "argus.ui", 5, json!({
                "operation": {
                    "type": "click",
                    "target": {
                        "scope": application_scope.clone(),
                        "locator": {
                            "type": "query",
                            "query": {
                                "language_version": 3,
                                "source": "nearest(anchor = text(name = \"最常使用\"), target = text(name = $contact_name), direction = below, index = 1)",
                                "bindings": { "contact_name": contact_input.clone() }
                            }
                        },
                        "backend_policy": {
                            "allow": ["ocr_small", "send_input"],
                            "deny": [],
                            "prefer": ["ocr_small", "send_input"]
                        }
                    }
                },
                "execution": {
                    "target_wait": { "mode": "bounded", "timeout_ms": 5000, "poll_interval_ms": 200 }
                }
            })),
            loop_node("wait_for_conversation", "wait_for_conversation_body"),
            fail_node(
                "conversation_not_ready",
                "wechat_conversation_not_ready",
                "未能打开联系人会话。请确认联系人名称后重试。",
            ),
            ui_node("type_message", text_operation(&application_scope, message_input.clone())),
            ui_node("send_message", enter_operation(&application_scope)),
            node(
                "wait_for_wechat_update",
                "argus.delay",
                1,
                json!({ "milliseconds": 800 }),
            ),
            loop_node("wait_for_send_result", "wait_for_send_result_body"),
            fail_node(
                "send_result_unknown",
                "wechat_send_result_unknown",
                "未能确认消息已发送。请打开微信检查当前会话后重试。",
            ),
            node("end", "argus.end", 1, json!({}))
            ],
            "edges": [
            edge("start", application_id, None),
            edge(application_id, "open_search", None),
            edge("open_search", "wait_for_search", None),
            edge("wait_for_search", "select_search_text", Some("completed")),
            edge("wait_for_search", "search_not_ready", Some("exhausted")),
            edge("select_search_text", "type_contact", None),
            edge("type_contact", "select_contact", None),
            edge("select_contact", "wait_for_conversation", None),
            edge("wait_for_conversation", "type_message", Some("completed")),
            edge("wait_for_conversation", "conversation_not_ready", Some("exhausted")),
            edge("type_message", "send_message", None),
            edge("send_message", "wait_for_wechat_update", None),
            edge("wait_for_wechat_update", "wait_for_send_result", None),
            edge("wait_for_send_result", "end", Some("completed")),
            edge("wait_for_send_result", "send_result_unknown", Some("exhausted")),
            ]
          },
          loop_scope(
              "wait_for_search_body",
              "wait_for_search",
              observe_node("check_search", &application_scope, search_ready, json!({})),
          ),
          loop_scope(
              "wait_for_conversation_body",
              "wait_for_conversation",
              observe_node(
                  "check_conversation",
                  &application_scope,
                  conversation_ready,
                  json!({ "contact_name": contact_input }),
              ),
          ),
          loop_scope(
              "wait_for_send_result_body",
              "wait_for_send_result",
              observe_node(
                  "check_send_result",
                  &application_scope,
                  message_sent,
                  json!({ "contact_name": input("联系人"), "message": message_input }),
              ),
          )]
        }
    })
}

/// 创建工作流中的开放节点对象。
fn node(id: &str, type_id: &str, version: u16, payload: Value) -> Value {
    json!({
        "id": id,
        "position": { "x": 0.0, "y": 0.0 },
        "size": { "width": 160.0, "height": 96.0 },
        "type_id": type_id,
        "version": version,
        "payload": payload,
        "output_bindings": {}
    })
}

/// 创建一条可选结果出口的稳定连线。
fn edge(source: &str, target: &str, branch: Option<&str>) -> Value {
    json!({
        "id": format!("edge_{source}_{}_{target}", branch.unwrap_or("next")),
        "source": source,
        "target": target,
        "branch": branch
    })
}

/// 创建一个有明确等待上限的重复执行节点。
fn loop_node(id: &str, body_scope_id: &str) -> Value {
    let mut node = node(
        id,
        "argus.loop",
        2,
        json!({
            "body_scope_id": body_scope_id,
            "max_iterations": 16,
            "timeout_ms": 5000,
            "interval_ms": 300
        }),
    );
    node["size"] = json!({ "width": 420.0, "height": 240.0 });
    node
}

/// 创建由强类型边界包围的一轮检查子作用域。
fn loop_scope(scope_id: &str, owner_node_id: &str, observation: Value) -> Value {
    let entry_id = format!("{scope_id}_entry");
    let continue_id = format!("{scope_id}_continue");
    let complete_id = format!("{scope_id}_complete");
    let observation_id = observation["id"]
        .as_str()
        .expect("观察节点必须有 ID")
        .to_owned();
    json!({
        "id": scope_id,
        "parent": { "scope_id": "root", "node_id": owner_node_id },
        "boundary": {
            "type": "loop",
            "entry_node_id": entry_id,
            "continue_node_id": continue_id,
            "complete_node_id": complete_id
        },
        "nodes": [
            node(&entry_id, "argus.loop.entry", 1, json!({})),
            observation,
            node(&continue_id, "argus.loop.continue", 1, json!({})),
            node(&complete_id, "argus.loop.complete", 1, json!({}))
        ],
        "edges": [
            edge(&entry_id, &observation_id, None),
            edge(&observation_id, &complete_id, Some("true")),
            edge(&observation_id, &continue_id, Some("false")),
            edge(&observation_id, &continue_id, Some("unknown"))
        ]
    })
}

/// 创建一个一次检查微信界面的布尔观察节点。
fn observe_node(id: &str, scope: &Value, source: &str, bindings: Value) -> Value {
    node(
        id,
        "argus.observe",
        1,
        json!({
            "observation": {
                "scope": scope,
                "query": { "language_version": 3, "source": source, "bindings": bindings },
                "backend_policy": {
                    "allow": ["ocr_small", "windows_uia"],
                    "deny": [],
                    "prefer": ["ocr_small", "windows_uia"]
                },
                "policy": { "mode": "once" }
            }
        }),
    )
}

/// 创建会停止工作流并提供恢复建议的失败终点。
fn fail_node(id: &str, code: &str, message: &str) -> Value {
    node(
        id,
        "argus.fail",
        1,
        json!({
            "code": code,
            "message": { "type": "literal", "value": message }
        }),
    )
}

/// 创建只依赖当前焦点的 UI v5 节点。
fn ui_node(id: &str, operation: Value) -> Value {
    node(
        id,
        "argus.ui",
        5,
        json!({
            "operation": operation,
            "execution": {
                "target_wait": { "mode": "none", "timeout_ms": 0, "poll_interval_ms": 0 }
            }
        }),
    )
}

/// 创建 Ctrl+字母组合键动作。
fn key_operation(scope: &Value, key: &str, modifiers: &[&str]) -> Value {
    json!({
        "type": "press_key",
        "target": input_target(scope),
        "chord": {
            "key": { "type": "character", "value": key },
            "modifiers": modifiers
        }
    })
}

/// 创建 Enter 发送动作。
fn enter_operation(scope: &Value) -> Value {
    json!({
        "type": "press_key",
        "target": input_target(scope),
        "chord": { "key": { "type": "enter" }, "modifiers": [] }
    })
}

/// 创建向当前焦点输入文字的动作。
fn text_operation(scope: &Value, value: Value) -> Value {
    json!({ "type": "type_text", "target": input_target(scope), "value": value })
}

/// 创建 SendInput 使用的微信当前焦点目标。
fn input_target(scope: &Value) -> Value {
    json!({
        "scope": scope,
        "locator": { "type": "focused" },
        "backend_policy": {
            "allow": ["send_input"],
            "deny": [],
            "prefer": ["send_input"]
        }
    })
}

/// 创建一个工作流输入引用。
fn input(key: &str) -> Value {
    json!({
        "type": "ref",
        "source": { "type": "workflow_input", "key": key },
        "pointer": ""
    })
}
