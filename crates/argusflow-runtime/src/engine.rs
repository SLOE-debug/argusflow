use std::{collections::HashMap, sync::Arc, time::Duration};

use argusflow_core::{
    ExecutionEvent, ExecutionEventKind, RunStarted, WorkflowDefinition, WorkflowNode,
    WorkflowNodeKind,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{ActionDispatcher, RuntimeError, validate_workflow};

/// 接收工作流执行事件的线程安全目标。
pub trait ExecutionEventSink: Send + Sync + 'static {
    /// 投递一个事件；返回错误会终止当前运行。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String>;
}

/// 管理单个活动运行并按线性连线执行工作流。
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

    /// 校验并异步启动一个工作流。
    ///
    /// 返回成功只表示运行已接受并排队；后续执行结果通过事件接收器报告。
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
                ExecutionEventKind::WorkflowStarted,
                Some(format!("开始执行工作流：{}", workflow.name)),
            ),
        )?;

        // 两个只读索引分别支持按 ID 取节点和沿源节点查找唯一后继，避免执行时反复扫描。
        let nodes: HashMap<&str, &WorkflowNode> = workflow
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let next_nodes: HashMap<&str, &str> = workflow
            .edges
            .iter()
            .map(|edge| (edge.source.as_str(), edge.target.as_str()))
            .collect();
        let mut current_id = workflow
            .nodes
            .iter()
            .find(|node| matches!(&node.kind, WorkflowNodeKind::Start))
            .map(|node| node.id.as_str());

        // 校验器已保证这是单入单出的链；运行时只需沿每条连线取唯一后继。
        while let Some(node_id) = current_id {
            let Some(node) = nodes.get(node_id) else {
                return Err(RuntimeError::ExecutionInvariant(format!(
                    "node '{node_id}' disappeared after validation"
                )));
            };
            emit(
                &sink,
                event(
                    run_id,
                    workflow.id,
                    &mut sequence,
                    Some(node.id.clone()),
                    ExecutionEventKind::NodeStarted,
                    Some(node_label(&node.kind)),
                ),
            )?;

            let execution_result = self
                .execute_node(node, &sink, run_id, workflow.id, &mut sequence)
                .await;
            // 节点失败要先发节点级事件，再发工作流级失败事件，保持消费者可重建状态。
            if let Err(error) = execution_result {
                let message = error.to_string();
                emit(
                    &sink,
                    event(
                        run_id,
                        workflow.id,
                        &mut sequence,
                        Some(node.id.clone()),
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
                    ExecutionEventKind::NodeSucceeded,
                    None,
                ),
            )?;

            current_id = next_nodes.get(node_id).copied();
        }

        emit(
            &sink,
            event(
                run_id,
                workflow.id,
                &mut sequence,
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
            WorkflowNodeKind::Start | WorkflowNodeKind::End => Ok(()),
            WorkflowNodeKind::Log { message } => emit(
                sink,
                event(
                    run_id,
                    workflow_id,
                    sequence,
                    Some(node.id.clone()),
                    ExecutionEventKind::Log,
                    Some(message.clone()),
                ),
            ),
            WorkflowNodeKind::Delay { milliseconds } => {
                // 校验阶段已限制毫秒数范围；这里直接休眠以保持执行语义简单且可预测。
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
                        ExecutionEventKind::Log,
                        Some(outcome.message),
                    ),
                )
            }
        }
    }
}

fn emit(sink: &Arc<dyn ExecutionEventSink>, event: ExecutionEvent) -> Result<(), RuntimeError> {
    // 将接收器的字符串错误统一转换为运行时错误，避免泄漏 sink 的具体实现。
    sink.emit(event).map_err(RuntimeError::EventSink)
}

fn event(
    run_id: Uuid,
    workflow_id: Uuid,
    sequence: &mut u64,
    node_id: Option<String>,
    kind: ExecutionEventKind,
    message: Option<String>,
) -> ExecutionEvent {
    // 序号在构造后递增，确保每个事件拿到唯一且连续的运行内序号。
    let event = ExecutionEvent {
        run_id,
        workflow_id,
        sequence: *sequence,
        node_id,
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
        WorkflowNodeKind::Action { .. } => "Action".to_owned(),
        WorkflowNodeKind::End => "End".to_owned(),
    }
}
