use std::{collections::HashMap, sync::Arc, time::Duration};

use argusflow_core::{
    ConditionBranch, ExecutionEvent, ExecutionEventKind, RunStarted, WorkflowDefinition,
    WorkflowEdge, WorkflowNode, WorkflowNodeKind,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{ActionDispatcher, RuntimeError, validate_workflow};

/// 接收工作流执行事件的线程安全目标。
pub trait ExecutionEventSink: Send + Sync + 'static {
    /// 投递事件；返回错误会终止当前运行。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String>;
}

/// 管理单个活动运行并按条件 DAG 的单一命中路径执行工作流。
pub struct WorkflowEngine {
    /// 当前活动运行 ID；同一引擎同时只允许一个运行。
    active_run: Mutex<Option<Uuid>>,
    /// 实际执行 Action 节点的后端调度器。
    dispatcher: Arc<dyn ActionDispatcher>,
}

impl WorkflowEngine {
    /// 创建使用指定动作调度器的工作流引擎。
    pub fn new(dispatcher: Arc<dyn ActionDispatcher>) -> Self {
        Self {
            active_run: Mutex::new(None),
            dispatcher,
        }
    }

    /// 查询当前活动运行；没有运行时返回 `None`。
    pub async fn active_run(&self) -> Option<Uuid> {
        *self.active_run.lock().await
    }

    /// 校验并异步启动一个工作流，执行结果通过事件接收器报告。
    pub async fn start(
        self: &Arc<Self>,
        workflow: WorkflowDefinition,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<RunStarted, RuntimeError> {
        let report = validate_workflow(&workflow);
        if !report.valid {
            return Err(RuntimeError::ValidationFailed { report });
        }
        let run_id = Uuid::new_v4();
        {
            let mut active_run = self.active_run.lock().await;
            // 在同一把锁内检查并写入，保证并发启动请求至多有一个成功。
            if let Some(run_id) = *active_run {
                return Err(RuntimeError::RunInProgress { run_id });
            }
            *active_run = Some(run_id);
        }
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let _ = engine.execute(run_id, workflow, sink).await;
            let mut active_run = engine.active_run.lock().await;
            if *active_run == Some(run_id) {
                *active_run = None;
            }
        });
        Ok(RunStarted { run_id })
    }

    async fn execute(
        &self,
        run_id: Uuid,
        workflow: WorkflowDefinition,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<(), RuntimeError> {
        // 事件序号从零开始，并由 event 在每次构造事件后统一递增。
        let mut sequence = 0;
        emit(
            &sink,
            event(
                run_id,
                workflow.id,
                &mut sequence,
                None,
                None,
                ExecutionEventKind::WorkflowStarted,
                Some(format!("开始执行工作流：{}", workflow.name)),
            ),
        )?;

        // 两个只读索引分别支持按 ID 取节点和沿源节点选择后继，避免执行时反复扫描。
        let nodes: HashMap<&str, &WorkflowNode> = workflow
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut outgoing: HashMap<&str, Vec<&WorkflowEdge>> = HashMap::new();
        for edge in &workflow.edges {
            outgoing.entry(edge.source.as_str()).or_default().push(edge);
        }
        let mut current_id = workflow
            .nodes
            .iter()
            .find(|node| matches!(&node.kind, WorkflowNodeKind::Start))
            .map(|node| node.id.as_str());

        while let Some(node_id) = current_id {
            let node = nodes.get(node_id).ok_or_else(|| {
                RuntimeError::ExecutionInvariant(format!(
                    "node '{node_id}' disappeared after validation"
                ))
            })?;
            emit(
                &sink,
                event(
                    run_id,
                    workflow.id,
                    &mut sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeStarted,
                    Some(node_label(&node.kind)),
                ),
            )?;
            if let Err(error) = self
                .execute_node(node, &sink, run_id, workflow.id, &mut sequence)
                .await
            {
                let message = error.to_string();
                emit(
                    &sink,
                    event(
                        run_id,
                        workflow.id,
                        &mut sequence,
                        Some(node.id.clone()),
                        None,
                        ExecutionEventKind::NodeFailed,
                        Some(message.clone()),
                    ),
                )?;
                emit(
                    &sink,
                    event(
                        run_id,
                        workflow.id,
                        &mut sequence,
                        None,
                        None,
                        ExecutionEventKind::WorkflowFailed,
                        Some(message),
                    ),
                )?;
                return Err(error);
            }
            emit(
                &sink,
                event(
                    run_id,
                    workflow.id,
                    &mut sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeSucceeded,
                    None,
                ),
            )?;

            let next_edge = match &node.kind {
                WorkflowNodeKind::Condition { predicate } => {
                    let matched = predicate.evaluate(&workflow.variables).map_err(|error| {
                        RuntimeError::ExecutionInvariant(error.to_string())
                    })?;
                    let branch = if matched {
                        ConditionBranch::True
                    } else {
                        ConditionBranch::False
                    };
                    outgoing
                        .get(node_id)
                        .into_iter()
                        .flatten()
                        .find(|edge| edge.branch == Some(branch))
                        .copied()
                }
                WorkflowNodeKind::Start
                | WorkflowNodeKind::Log { .. }
                | WorkflowNodeKind::Delay { .. }
                | WorkflowNodeKind::Action { .. } => outgoing
                    .get(node_id)
                    .and_then(|edges| edges.first())
                    .copied(),
                WorkflowNodeKind::End => None,
            };
            if let Some(edge) = next_edge {
                emit(
                    &sink,
                    event(
                        run_id,
                        workflow.id,
                        &mut sequence,
                        None,
                        Some(edge.id.clone()),
                        ExecutionEventKind::EdgeTraversed,
                        Some(format!("{} → {}", edge.source, edge.target)),
                    ),
                )?;
                current_id = Some(edge.target.as_str());
            } else {
                current_id = None;
            }
        }
        emit(
            &sink,
            event(
                run_id,
                workflow.id,
                &mut sequence,
                None,
                None,
                ExecutionEventKind::WorkflowCompleted,
                Some("工作流执行完成".to_owned()),
            ),
        )?;
        Ok(())
    }

    async fn execute_node(
        &self,
        node: &WorkflowNode,
        sink: &Arc<dyn ExecutionEventSink>,
        run_id: Uuid,
        workflow_id: Uuid,
        sequence: &mut u64,
    ) -> Result<(), RuntimeError> {
        match &node.kind {
            WorkflowNodeKind::Start
            | WorkflowNodeKind::End
            | WorkflowNodeKind::Condition { .. } => Ok(()),
            WorkflowNodeKind::Log { message } => emit(
                sink,
                event(
                    run_id,
                    workflow_id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::Log,
                    Some(message.clone()),
                ),
            ),
            WorkflowNodeKind::Delay { milliseconds } => {
                tokio::time::sleep(Duration::from_millis(*milliseconds)).await;
                Ok(())
            }
            WorkflowNodeKind::Action { action } => {
                let outcome = self.dispatcher.execute(action).await?;
                emit(
                    sink,
                    event(
                        run_id,
                        workflow_id,
                        sequence,
                        Some(node.id.clone()),
                        None,
                        ExecutionEventKind::Log,
                        Some(outcome.message),
                    ),
                )
            }
        }
    }
}

fn emit(
    sink: &Arc<dyn ExecutionEventSink>,
    event: ExecutionEvent,
) -> Result<(), RuntimeError> {
    sink.emit(event).map_err(RuntimeError::EventSink)
}

fn event(
    run_id: Uuid,
    workflow_id: Uuid,
    sequence: &mut u64,
    node_id: Option<String>,
    edge_id: Option<String>,
    kind: ExecutionEventKind,
    message: Option<String>,
) -> ExecutionEvent {
    let event = ExecutionEvent {
        run_id,
        workflow_id,
        sequence: *sequence,
        node_id,
        edge_id,
        kind,
        message,
    };
    *sequence += 1;
    event
}

fn node_label(kind: &WorkflowNodeKind) -> String {
    match kind {
        WorkflowNodeKind::Start => "Start".to_owned(),
        WorkflowNodeKind::Log { .. } => "Log".to_owned(),
        WorkflowNodeKind::Delay { milliseconds } => format!("Delay {milliseconds}ms"),
        WorkflowNodeKind::Condition { .. } => "Condition".to_owned(),
        WorkflowNodeKind::Action { .. } => "Action".to_owned(),
        WorkflowNodeKind::End => "End".to_owned(),
    }
}
