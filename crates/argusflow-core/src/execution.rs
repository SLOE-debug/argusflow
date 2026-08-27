use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 工作流启动后返回给调用方的运行标识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStarted {
    /// 本次运行的唯一标识。
    pub run_id: Uuid,
}

/// 运行时按序发出的工作流执行事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// 本次运行的唯一标识。
    pub run_id: Uuid,
    /// 事件所属工作流的唯一标识。
    pub workflow_id: Uuid,
    /// 从零开始、在单次运行内严格递增的事件序号。
    pub sequence: u64,
    /// 相关节点 ID；工作流级事件不绑定具体节点。
    pub node_id: Option<String>,
    /// 组件内部执行时的扁平节点 ID；普通节点事件为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded_node_id: Option<String>,
    /// 组件内部执行时从外到内的版本锁定来源路径。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_path: Vec<ExecutionComponentFrame>,
    /// 相关连线 ID；只有连线流转事件携带该字段。
    pub edge_id: Option<String>,
    /// 事件的生命周期类别。
    pub kind: ExecutionEventKind,
    /// 可选的人类可读消息或错误摘要。
    pub message: Option<String>,
    /// 不包含完整业务数据或 OS handle 的结构化事件载荷。
    pub payload: Option<ExecutionEventPayload>,
}

/// 执行事件中一个不包含业务值的组件来源帧。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionComponentFrame {
    /// 当前层组件实例在展开图中的节点 ID。
    pub instance_node_id: String,
    /// 当前层组件稳定 UUID。
    pub component_id: Uuid,
    /// 当前层实例锁定的精确版本。
    pub component_version: String,
    /// 执行节点在当前层定义中的直接内部节点 ID。
    pub inner_node_id: String,
}

/// 执行事件中可安全展示的结构化详情。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEventPayload {
    /// 节点已经保存一组值输出，仅报告端口名避免泄露业务内容。
    NodeOutputsProduced {
        /// 本次产生的稳定输出端口名称。
        output_names: Vec<String>,
    },
    /// 应用节点已经获取逻辑资源，不暴露 PID 或 HWND。
    ResourceAcquired {
        /// 资源输出端口名称。
        output_name: String,
        /// 稳定的资源类别名称。
        resource_type: String,
    },
    /// Planner 已选择语义动作后端。
    BackendSelected {
        /// 实际执行操作的后端。
        backend: crate::BackendKind,
    },
    /// Command 节点已经退出；stdout/stderr 只保存在 NodeOutcome。
    CommandExited {
        /// 子进程退出代码。
        exit_code: i32,
    },
    /// 已持久化 Failure Evidence，只报告稳定引用和分支摘要。
    DiagnosticEvidenceCaptured {
        /// artifact sink 生成的 evidence 标识。
        evidence_id: Uuid,
        /// 产生证据的自动化后端。
        backend: crate::BackendKind,
        /// 失败 candidate 的 AQL fallback 路径。
        branch_path: Vec<usize>,
        /// 是否由更晚 candidate 恢复成功。
        recovered_by_fallback: bool,
    },
}

/// 工作流和节点生命周期中可观察的事件类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEventKind {
    /// 工作流已开始执行。
    WorkflowStarted,
    /// 某个节点已开始执行。
    NodeStarted,
    /// 节点产生了日志消息。
    Log,
    /// 节点产生了一个或多个结构化输出。
    NodeOutputProduced,
    /// 资源节点成功获取了运行时资源。
    ResourceAcquired,
    /// Planner 已为语义 UI 操作选择实际执行后端。
    BackendSelected,
    /// Command 子进程已经以可接受的退出代码结束。
    CommandExited,
    /// 自动化候选失败现场已写入 artifact store。
    DiagnosticEvidenceCaptured,
    /// 某个节点执行成功。
    NodeSucceeded,
    /// 运行时已选择并进入一条连线。
    EdgeTraversed,
    /// 某个节点执行失败。
    NodeFailed,
    /// 工作流中的所有节点已完成。
    WorkflowCompleted,
    /// 工作流因节点或运行时错误而失败。
    WorkflowFailed,
}
