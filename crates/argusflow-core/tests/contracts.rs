//! 核心工作流数据契约的序列化回归测试。
//!
//! 通过 JSON 往返确认编辑器与运行时共享的结构能够无损持久化和恢复。

use argusflow_core::{
    AutomationAction, Position, Selector, WorkflowDefinition, WorkflowEdge, WorkflowNode,
    WorkflowNodeKind,
};
use uuid::Uuid;
use serde_json::json;

#[test]
fn workflow_contract_round_trips_through_json() {
    // 使用包含动作选择器和多条连线的最小完整工作流，覆盖扁平化节点类型及嵌套枚举。
    let workflow = WorkflowDefinition {
        schema_version: 2,
        id: Uuid::new_v4(),
        name: "契约测试".to_owned(),
        variables: json!({ "enabled": true }),
        nodes: vec![
            WorkflowNode {
                id: "start".to_owned(),
                position: Position { x: 0.0, y: 0.0 },
                kind: WorkflowNodeKind::Start,
            },
            WorkflowNode {
                id: "action".to_owned(),
                position: Position { x: 240.0, y: 0.0 },
                kind: WorkflowNodeKind::Action {
                    action: AutomationAction::Click {
                        target: Selector::Native {
                            name: Some("保存".to_owned()),
                            automation_id: None,
                            control_type: Some("Button".to_owned()),
                        },
                    },
                },
            },
            WorkflowNode {
                id: "end".to_owned(),
                position: Position { x: 480.0, y: 0.0 },
                kind: WorkflowNodeKind::End,
            },
        ],
        edges: vec![
            WorkflowEdge {
                id: "start-action".to_owned(),
                source: "start".to_owned(),
                target: "action".to_owned(),
                branch: None,
            },
            WorkflowEdge {
                id: "action-end".to_owned(),
                source: "action".to_owned(),
                target: "end".to_owned(),
                branch: None,
            },
        ],
    };

    let json = serde_json::to_string(&workflow).expect("workflow should serialize");
    let decoded: WorkflowDefinition =
        serde_json::from_str(&json).expect("workflow should deserialize");

    assert_eq!(decoded, workflow);
}
