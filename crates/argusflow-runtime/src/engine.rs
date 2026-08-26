use std::{collections::HashMap, sync::Arc};

use argusflow_core::{
    ApplicationSessionProvider, ConditionBranch, ExecutionEvent, ExecutionEventKind,
    ExecutionEventPayload, RunInputs, RunStarted, WorkflowDefinition, WorkflowEdge, WorkflowNode,
    WorkflowNodeKind,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    ActionDispatcher, RunContext, RuntimeError, UnavailableApplicationSessionProvider,
    node_executor::WorkflowNodeExecutor, run_inputs::validate_run_inputs, validate_workflow,
};

/// 接收工作流执行事件的线程安全目标。
pub trait ExecutionEventSink: Send + Sync + 'static {
    /// 投递事件；返回错误会终止当前运行。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String>;
}

/// 管理单个活动运行并按条件 DAG 的单一命中路径执行工作流。
pub struct WorkflowEngine {
    /// 当前活动运行 ID；同一引擎同时只允许一个运行。
    active_run: Mutex<Option<Uuid>>,
    /// 资源、UI 和命令节点的强类型执行编排器。
    nodes: WorkflowNodeExecutor,
}

impl WorkflowEngine {
    /// 创建没有平台应用资源能力的工作流引擎。
    pub fn new(dispatcher: Arc<dyn ActionDispatcher>) -> Self {
        Self::with_application_provider(dispatcher, Arc::new(UnavailableApplicationSessionProvider))
    }

    /// 创建同时装配 UI Planner 与平台应用资源能力的工作流引擎。
    pub fn with_application_provider(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
    ) -> Self {
        Self {
            active_run: Mutex::new(None),
            nodes: WorkflowNodeExecutor::new(dispatcher, applications),
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
        inputs: RunInputs,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<RunStarted, RuntimeError> {
        let report = validate_workflow(&workflow);
        if !report.valid {
            return Err(RuntimeError::ValidationFailed { report });
        }
        validate_run_inputs(&workflow, &inputs)?;
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
            let _ = engine.execute(run_id, workflow, inputs, sink).await;
            let mut active_run = engine.active_run.lock().await;
            if *active_run == Some(run_id) {
                *active_run = None;
            }
        });
        Ok(RunStarted { run_id })
    }

    /// 执行单一命中路径，并保证已经获取的资源在成功或失败后都进入清理阶段。
    async fn execute(
        &self,
        run_id: Uuid,
        workflow: WorkflowDefinition,
        inputs: RunInputs,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<(), RuntimeError> {
        // Validator 已保证 variables 是对象；这里保留结构约束错误以防未来绕过入口。
        let variables = workflow.variables.as_object().cloned().ok_or_else(|| {
            RuntimeError::ExecutionInvariant("workflow variables are not an object".to_owned())
        })?;
        let mut context = RunContext::new(run_id, inputs.values, variables);
        let mut sequence = 0;
        emit_event(
            &sink,
            build_event(
                run_id,
                workflow.id,
                &mut sequence,
                None,
                None,
                ExecutionEventKind::WorkflowStarted,
                Some(format!("开始执行工作流：{}", workflow.name)),
                None,
            ),
        )?;

        let execution = self
            .execute_path(&workflow, &sink, &mut context, &mut sequence)
            .await;
        let cleanup = self.nodes.cleanup(&context).await;
        let result = execution.and(cleanup);
        match result {
            Ok(()) => emit_event(
                &sink,
                build_event(
                    run_id,
                    workflow.id,
                    &mut sequence,
                    None,
                    None,
                    ExecutionEventKind::WorkflowCompleted,
                    Some("工作流执行完成".to_owned()),
                    None,
                ),
            ),
            Err(error) => {
                emit_event(
                    &sink,
                    build_event(
                        run_id,
                        workflow.id,
                        &mut sequence,
                        None,
                        None,
                        ExecutionEventKind::WorkflowFailed,
                        Some(error.to_string()),
                        None,
                    ),
                )?;
                Err(error)
            }
        }
    }

    /// 沿控制流执行节点，并把 NodeOutcome 持久化到 RunContext 数据面。
    async fn execute_path(
        &self,
        workflow: &WorkflowDefinition,
        sink: &Arc<dyn ExecutionEventSink>,
        context: &mut RunContext,
        sequence: &mut u64,
    ) -> Result<(), RuntimeError> {
        // 两个只读索引支持按 ID 取节点和沿源节点选择后继，避免执行时反复扫描。
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
            emit_event(
                sink,
                build_event(
                    context.run_id,
                    workflow.id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeStarted,
                    Some(node_label(&node.kind)),
                    None,
                ),
            )?;
            let execution = match self
                .nodes
                .execute(node, workflow.permissions, context)
                .await
            {
                Ok(execution) => execution,
                Err(error) => {
                    emit_event(
                        sink,
                        build_event(
                            context.run_id,
                            workflow.id,
                            sequence,
                            Some(node.id.clone()),
                            None,
                            ExecutionEventKind::NodeFailed,
                            Some(error.to_string()),
                            None,
                        ),
                    )?;
                    return Err(error);
                }
            };
            context.record_outcome(node.id.clone(), execution.outcome);
            for node_event in execution.events {
                emit_event(
                    sink,
                    build_event(
                        context.run_id,
                        workflow.id,
                        sequence,
                        Some(node.id.clone()),
                        None,
                        node_event.kind,
                        node_event.message,
                        node_event.payload,
                    ),
                )?;
            }
            emit_event(
                sink,
                build_event(
                    context.run_id,
                    workflow.id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeSucceeded,
                    None,
                    None,
                ),
            )?;

            let next_edge = select_next_edge(node, node_id, &outgoing, &workflow.variables)?;
            if let Some(edge) = next_edge {
                emit_event(
                    sink,
                    build_event(
                        context.run_id,
                        workflow.id,
                        sequence,
                        None,
                        Some(edge.id.clone()),
                        ExecutionEventKind::EdgeTraversed,
                        Some(format!("{} → {}", edge.source, edge.target)),
                        None,
                    ),
                )?;
                current_id = Some(edge.target.as_str());
            } else {
                current_id = None;
            }
        }
        Ok(())
    }
}

/// 根据普通或条件节点选择唯一后继边。
fn select_next_edge<'a>(
    node: &WorkflowNode,
    node_id: &str,
    outgoing: &'a HashMap<&str, Vec<&'a WorkflowEdge>>,
    variables: &serde_json::Value,
) -> Result<Option<&'a WorkflowEdge>, RuntimeError> {
    match &node.kind {
        WorkflowNodeKind::Condition { predicate } => {
            let matched = predicate
                .evaluate(variables)
                .map_err(|error| RuntimeError::ExecutionInvariant(error.to_string()))?;
            let branch = if matched {
                ConditionBranch::True
            } else {
                ConditionBranch::False
            };
            Ok(outgoing
                .get(node_id)
                .into_iter()
                .flatten()
                .find(|edge| edge.branch == Some(branch))
                .copied())
        }
        WorkflowNodeKind::End => Ok(None),
        WorkflowNodeKind::Start
        | WorkflowNodeKind::Log { .. }
        | WorkflowNodeKind::Debug { .. }
        | WorkflowNodeKind::Delay { .. }
        | WorkflowNodeKind::Application { .. }
        | WorkflowNodeKind::Ui { .. }
        | WorkflowNodeKind::Command { .. } => Ok(outgoing
            .get(node_id)
            .and_then(|edges| edges.first())
            .copied()),
    }
}

/// 将事件交付错误统一映射到 RuntimeError。
fn emit_event(
    sink: &Arc<dyn ExecutionEventSink>,
    event: ExecutionEvent,
) -> Result<(), RuntimeError> {
    sink.emit(event).map_err(RuntimeError::EventSink)
}

/// 构造严格递增序号的执行事件。
#[allow(clippy::too_many_arguments)]
fn build_event(
    run_id: Uuid,
    workflow_id: Uuid,
    sequence: &mut u64,
    node_id: Option<String>,
    edge_id: Option<String>,
    kind: ExecutionEventKind,
    message: Option<String>,
    payload: Option<ExecutionEventPayload>,
) -> ExecutionEvent {
    let event = ExecutionEvent {
        run_id,
        workflow_id,
        sequence: *sequence,
        node_id,
        edge_id,
        kind,
        message,
        payload,
    };
    *sequence += 1;
    event
}

/// 返回节点启动事件使用的稳定摘要。
fn node_label(kind: &WorkflowNodeKind) -> String {
    match kind {
        WorkflowNodeKind::Start => "Start".to_owned(),
        WorkflowNodeKind::Log { .. } => "Log".to_owned(),
        WorkflowNodeKind::Debug { .. } => "Debug Output".to_owned(),
        WorkflowNodeKind::Delay { milliseconds } => format!("Delay {milliseconds}ms"),
        WorkflowNodeKind::Condition { .. } => "Condition".to_owned(),
        WorkflowNodeKind::Application { .. } => "Application".to_owned(),
        WorkflowNodeKind::Ui { operation } => ui_node_label(operation).to_owned(),
        WorkflowNodeKind::Command { operation } => format!("Command {:?}", operation.runner),
        WorkflowNodeKind::End => "End".to_owned(),
    }
}

/// 返回不会包含 SetValue 业务数据的 UI 节点摘要。
fn ui_node_label(operation: &argusflow_core::UiOperation) -> &'static str {
    match operation {
        argusflow_core::UiOperation::Click { .. } => "UI Click",
        argusflow_core::UiOperation::SetValue { .. } => "UI SetValue",
        argusflow_core::UiOperation::GetText { .. } => "UI GetText",
        argusflow_core::UiOperation::GetValue { .. } => "UI GetValue",
    }
}
