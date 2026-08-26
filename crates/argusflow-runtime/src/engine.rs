use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{
    ApplicationSessionProvider, BrowserSessionProvider, ExecutionEvent, ExecutionEventKind,
    ExecutionEventPayload, RunInputs, RunStarted, WorkflowEdge, WorkflowNode,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    ActionDispatcher, NodeCompiler, NodeFlow, NodeRegistryError, NodeTypeRegistry, PreparedNode,
    RunContext, RuntimeError, UnavailableApplicationSessionProvider,
    UnavailableBrowserSessionProvider, builtin_nodes,
    run_inputs::validate_run_inputs,
    scheduler::ResourceScheduler,
    validator::{PreparedWorkflow, prepare_workflow},
};

/// 接收工作流执行事件的线程安全目标。
pub trait ExecutionEventSink: Send + Sync + 'static {
    /// 投递事件；返回错误会终止当前运行。
    fn emit(&self, event: ExecutionEvent) -> Result<(), String>;
}

/// 管理多个 RunWorld，并执行已经由开放注册表冻结的强类型节点路径。
pub struct WorkflowEngine {
    /// 当前活动 RunWorld 集合；不同资源的节点可以并行推进。
    active_runs: Mutex<HashSet<Uuid>>,
    /// 在启动前把 NodeEnvelope 编译为 PreparedNode 的注册表。
    node_types: NodeTypeRegistry,
    /// 跨 RunWorld 按资源 read/exclusive 语义仲裁副作用。
    scheduler: ResourceScheduler,
}

impl WorkflowEngine {
    /// 创建没有平台资源能力、但包含全部内置节点定义的工作流引擎。
    pub fn new(dispatcher: Arc<dyn ActionDispatcher>) -> Self {
        Self::with_resource_providers(
            dispatcher,
            Arc::new(UnavailableApplicationSessionProvider),
            Arc::new(UnavailableBrowserSessionProvider),
        )
    }

    /// 创建同时装配 UI Planner 与平台应用资源能力的工作流引擎。
    pub fn with_application_provider(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
    ) -> Self {
        Self::with_resource_providers(
            dispatcher,
            applications,
            Arc::new(UnavailableBrowserSessionProvider),
        )
    }

    /// 创建同时装配 UI Planner、桌面应用与浏览器资源能力的工作流引擎。
    pub fn with_resource_providers(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
        browsers: Arc<dyn BrowserSessionProvider>,
    ) -> Self {
        Self {
            active_runs: Mutex::new(HashSet::new()),
            node_types: builtin_nodes::registry(dispatcher, applications, browsers),
            scheduler: ResourceScheduler::default(),
        }
    }

    /// 创建内置节点运行时，并追加宿主提供的开放节点类型编译器。
    pub fn with_node_compilers(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
        browsers: Arc<dyn BrowserSessionProvider>,
        compilers: impl IntoIterator<Item = Arc<dyn NodeCompiler>>,
    ) -> Result<Self, NodeRegistryError> {
        let mut node_types = builtin_nodes::registry(dispatcher, applications, browsers);
        for compiler in compilers {
            node_types.register(compiler)?;
        }
        Ok(Self {
            active_runs: Mutex::new(HashSet::new()),
            node_types,
            scheduler: ResourceScheduler::default(),
        })
    }

    /// 返回稳定排序的全部活动 RunWorld ID。
    pub async fn active_runs(&self) -> Vec<Uuid> {
        let mut runs = self
            .active_runs
            .lock()
            .await
            .iter()
            .copied()
            .collect::<Vec<_>>();
        runs.sort_unstable();
        runs
    }

    /// 编译、校验并异步启动工作流，执行结果通过事件接收器报告。
    pub async fn start(
        self: &Arc<Self>,
        workflow: argusflow_core::WorkflowDefinition,
        inputs: RunInputs,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<RunStarted, RuntimeError> {
        let workflow = prepare_workflow(workflow, &self.node_types)
            .map_err(|report| RuntimeError::ValidationFailed { report })?;
        validate_run_inputs(&workflow.definition, &inputs)?;
        let run_id = Uuid::new_v4();
        self.active_runs.lock().await.insert(run_id);
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let _ = engine.execute(run_id, workflow, inputs, sink).await;
            engine.active_runs.lock().await.remove(&run_id);
        });
        Ok(RunStarted { run_id })
    }

    /// 执行单一命中路径，并保证所有注册资源都进入各自冻结的清理策略。
    async fn execute(
        &self,
        run_id: Uuid,
        workflow: PreparedWorkflow,
        inputs: RunInputs,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<(), RuntimeError> {
        // Validator 已保证 variables 是对象；这里保留结构约束错误以防未来绕过入口。
        let variables = workflow
            .definition
            .variables
            .as_object()
            .cloned()
            .ok_or_else(|| {
                RuntimeError::ExecutionInvariant("workflow variables are not an object".to_owned())
            })?;
        let mut context = RunContext::new(run_id, inputs.values, variables);
        let mut sequence = 0;
        emit_event(
            &sink,
            build_event(
                run_id,
                workflow.definition.id,
                &mut sequence,
                None,
                None,
                ExecutionEventKind::WorkflowStarted,
                Some(format!("开始执行工作流：{}", workflow.definition.name)),
                None,
            ),
        )?;

        let execution = self
            .execute_path(&workflow, &sink, &mut context, &mut sequence)
            .await;
        let cleanup_access = context.resources().cleanup_access_set();
        let _cleanup_guard = self.scheduler.acquire(cleanup_access).await;
        let cleanup = context.resources().cleanup_all().await;
        let result = execution.and(cleanup);
        match result {
            Ok(()) => emit_event(
                &sink,
                build_event(
                    run_id,
                    workflow.definition.id,
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
                        workflow.definition.id,
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

    /// 沿控制流执行 PreparedNode，并把 NodeOutcome 保存到 RunContext 数据面。
    async fn execute_path(
        &self,
        workflow: &PreparedWorkflow,
        sink: &Arc<dyn ExecutionEventSink>,
        context: &mut RunContext,
        sequence: &mut u64,
    ) -> Result<(), RuntimeError> {
        // 两个只读索引支持按 ID 取 definition 和沿源节点选择后继。
        let nodes: HashMap<&str, &WorkflowNode> = workflow
            .definition
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut outgoing: HashMap<&str, Vec<&WorkflowEdge>> = HashMap::new();
        for edge in &workflow.definition.edges {
            outgoing.entry(edge.source.as_str()).or_default().push(edge);
        }
        let mut current_id = workflow
            .definition
            .nodes
            .iter()
            .find(|node| {
                workflow
                    .nodes
                    .get(&node.id)
                    .is_some_and(|prepared| matches!(prepared.flow(), NodeFlow::Start))
            })
            .map(|node| node.id.as_str());

        while let Some(node_id) = current_id {
            let node = nodes.get(node_id).ok_or_else(|| {
                RuntimeError::ExecutionInvariant(format!(
                    "node '{node_id}' disappeared after validation"
                ))
            })?;
            let prepared = workflow.nodes.get(node_id).ok_or_else(|| {
                RuntimeError::ExecutionInvariant(format!(
                    "node '{node_id}' has no prepared execution after validation"
                ))
            })?;
            emit_event(
                sink,
                build_event(
                    context.run_id,
                    workflow.definition.id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeStarted,
                    Some(prepared.label()),
                    None,
                ),
            )?;
            let access = prepared.access_set(&node.id, context)?;
            let access_guard = self.scheduler.acquire(access).await;
            let execution = match prepared
                .execute(&node.id, &workflow.definition.permissions, context)
                .await
            {
                Ok(execution) => execution,
                Err(error) => {
                    emit_event(
                        sink,
                        build_event(
                            context.run_id,
                            workflow.definition.id,
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
            drop(access_guard);
            context.record_outcome(node.id.clone(), execution.outcome);
            for node_event in execution.events {
                emit_event(
                    sink,
                    build_event(
                        context.run_id,
                        workflow.definition.id,
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
                    workflow.definition.id,
                    sequence,
                    Some(node.id.clone()),
                    None,
                    ExecutionEventKind::NodeSucceeded,
                    None,
                    None,
                ),
            )?;

            let next_edge = select_next_edge(
                prepared.as_ref(),
                node_id,
                &outgoing,
                &workflow.definition.variables,
            )?;
            if let Some(edge) = next_edge {
                emit_event(
                    sink,
                    build_event(
                        context.run_id,
                        workflow.definition.id,
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

/// 根据 PreparedNode 声明的控制流与可选分支选择唯一后继边。
fn select_next_edge<'edge>(
    node: &dyn PreparedNode,
    node_id: &str,
    outgoing: &'edge HashMap<&str, Vec<&'edge WorkflowEdge>>,
    variables: &serde_json::Value,
) -> Result<Option<&'edge WorkflowEdge>, RuntimeError> {
    if matches!(node.flow(), NodeFlow::End) {
        return Ok(None);
    }
    let selected_branch = node.select_branch(variables)?;
    Ok(outgoing
        .get(node_id)
        .into_iter()
        .flatten()
        .find(|edge| edge.branch == selected_branch)
        .copied())
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
