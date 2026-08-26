//! 开放节点注册边界的契约测试。

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use argusflow_core::{
    ExecutionEvent, ExecutionEventKind, NodeEnvelope, NodeTypeId, Position, RunInputs,
    WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowPermissions,
};
use argusflow_runtime::{
    ExecutionEventSink, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution, NodeFlow,
    NodeTypeRegistry, NodeValidationContext, PreparedNode, RunContext, RuntimeError,
    UnavailableActionDispatcher, UnavailableApplicationSessionProvider,
    UnavailableBrowserSessionProvider, ValidationIssue, ValidationIssueCode, WorkflowEngine,
    validate_workflow_with_registry,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

/// 测试宿主注册的自定义节点 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmitPayload {
    /// 写入执行事件的固定文本。
    message: String,
}

/// 证明宿主模块可以在不修改 Runtime 中央分发的情况下注册新节点。
struct EmitCompiler {
    /// 注册表使用的命名空间化节点类型。
    type_id: NodeTypeId,
    /// 记录强类型节点确实进入执行路径。
    executions: Arc<AtomicUsize>,
}

impl NodeCompiler for EmitCompiler {
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        if definition.version != 1 {
            return Err(NodeCompileError::new("test.emit only supports version 1"));
        }
        let payload = serde_json::from_value::<EmitPayload>(definition.payload.clone())
            .map_err(|error| NodeCompileError::new(error.to_string()))?;
        Ok(Arc::new(EmitNode {
            message: payload.message,
            executions: Arc::clone(&self.executions),
        }))
    }
}

/// 自定义节点解码后的冻结执行对象。
#[derive(Debug)]
struct EmitNode {
    /// 只在 prepare 阶段从 JSON 解码一次的强类型消息。
    message: String,
    /// 测试观察用执行计数器。
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl PreparedNode for EmitNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Test Emit".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        if self.message.trim().is_empty() {
            vec![context.issue(
                ValidationIssueCode::custom("test.emit.empty_message"),
                "测试扩展消息不能为空",
            )]
        } else {
            Vec::new()
        }
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        _context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(NodeExecution {
            events: vec![NodeEvent {
                kind: ExecutionEventKind::Log,
                message: Some(self.message.clone()),
                payload: None,
            }],
            ..NodeExecution::default()
        })
    }
}

/// 将运行事件转发给测试接收端。
struct ChannelSink(mpsc::UnboundedSender<ExecutionEvent>);

impl ExecutionEventSink for ChannelSink {
    fn emit(&self, event: ExecutionEvent) -> Result<(), String> {
        self.0.send(event).map_err(|error| error.to_string())
    }
}

#[test]
fn custom_validation_codes_preserve_their_namespace() {
    let mut registry = NodeTypeRegistry::new();
    registry
        .register(Arc::new(EmitCompiler {
            type_id: NodeTypeId::new("test.emit"),
            executions: Arc::new(AtomicUsize::new(0)),
        }))
        .expect("custom type should register");
    let workflow = single_node_workflow("");

    let report = validate_workflow_with_registry(&workflow, &registry);

    assert!(
        report
            .issues
            .iter()
            .any(|issue| { issue.code.as_str() == "test.emit.empty_message" })
    );
}

#[tokio::test]
async fn engine_executes_a_registered_node_without_central_dispatch_changes() {
    let executions = Arc::new(AtomicUsize::new(0));
    let compiler: Arc<dyn NodeCompiler> = Arc::new(EmitCompiler {
        type_id: NodeTypeId::new("test.emit"),
        executions: Arc::clone(&executions),
    });
    let engine = Arc::new(
        WorkflowEngine::with_node_compilers(
            Arc::new(UnavailableActionDispatcher),
            Arc::new(UnavailableApplicationSessionProvider),
            Arc::new(UnavailableBrowserSessionProvider),
            [compiler],
        )
        .expect("custom type should not conflict with built-ins"),
    );
    let (sender, mut receiver) = mpsc::unbounded_channel();

    engine
        .start(
            single_node_workflow("extension executed"),
            RunInputs::default(),
            Arc::new(ChannelSink(sender)),
        )
        .await
        .expect("registered workflow should start");

    let mut observed_message = false;
    while let Some(event) = receiver.recv().await {
        observed_message |= event.kind == ExecutionEventKind::Log
            && event.message.as_deref() == Some("extension executed");
        if event.kind == ExecutionEventKind::WorkflowCompleted {
            break;
        }
    }
    assert!(observed_message);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

/// 创建 start -> test.emit -> end 的最小开放节点工作流。
fn single_node_workflow(message: &str) -> WorkflowDefinition {
    WorkflowDefinition {
        schema_version: 7,
        id: Uuid::new_v4(),
        name: "节点扩展契约".to_owned(),
        inputs: Vec::new(),
        variables: json!({}),
        permissions: WorkflowPermissions::default(),
        nodes: vec![
            node("start", "argus.start", json!({})),
            node("emit", "test.emit", json!({ "message": message })),
            node("end", "argus.end", json!({})),
        ],
        edges: vec![
            edge("start-emit", "start", "emit"),
            edge("emit-end", "emit", "end"),
        ],
    }
}

/// 创建 schema v7 节点定义。
fn node(id: &str, type_id: &str, payload: serde_json::Value) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        position: Position { x: 0.0, y: 0.0 },
        definition: NodeEnvelope::new(type_id, 1, payload),
    }
}

/// 创建没有分支标签的线性连线。
fn edge(id: &str, source: &str, target: &str) -> WorkflowEdge {
    WorkflowEdge {
        id: id.to_owned(),
        source: source.to_owned(),
        target: target.to_owned(),
        branch: None,
    }
}
