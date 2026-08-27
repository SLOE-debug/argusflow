use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use argusflow_core::{
    ApplicationSessionProvider, BrowserSessionProvider, ExecutionEvent, ExecutionEventKind,
    ExecutionEventPayload, FlowComponentDefinition, RunInputs, RunStarted, WorkflowEdge,
    WorkflowNode,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    ActionDispatcher, ComponentRegistry, NodeCompiler, NodeFlow, NodeRegistryError,
    NodeTypeRegistry, PreparedNode, RunContext, RuntimeError,
    UnavailableApplicationSessionProvider, UnavailableBrowserSessionProvider, builtin_nodes,
    execution_events::{build_event, emit_event},
    expand_components,
    run_inputs::validate_run_inputs,
    scheduler::ResourceScheduler,
    validator::{PreparedWorkflow, prepare_workflow},
    value_runtime::publish_outcome,
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
        self.start_with_components(workflow, Vec::new(), inputs, sink)
            .await
    }

    /// 解析版本锁定组件、编译并异步启动工作流。
    pub async fn start_with_components(
        self: &Arc<Self>,
        workflow: argusflow_core::WorkflowDefinition,
        components: Vec<FlowComponentDefinition>,
        inputs: RunInputs,
        sink: Arc<dyn ExecutionEventSink>,
    ) -> Result<RunStarted, RuntimeError> {
        let component_registry =
            ComponentRegistry::from_definitions(components).map_err(|error| {
                RuntimeError::ValidationFailed {
                    report: component_validation_report(error.to_string(), None),
                }
            })?;
        let expanded = expand_components(workflow, &component_registry).map_err(|error| {
            RuntimeError::ValidationFailed {
                report: component_validation_report(error.message, error.node_id),
            }
        })?;
        let workflow = prepare_workflow(expanded.definition, &self.node_types, expanded.source_map)
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
        let mut context = RunContext::with_value_plan(
            run_id,
            inputs.values,
            variables,
            Arc::clone(&workflow.value_plan),
        );
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
            &workflow.source_map,
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
                &workflow.source_map,
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
                    &workflow.source_map,
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
                &workflow.source_map,
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
                        &workflow.source_map,
                    )?;
                    return Err(error);
                }
            };
            drop(access_guard);
            let published_outcome = match publish_outcome(
                context,
                &node.id,
                execution.outcome,
                &node.output_bindings,
            ) {
                Ok(outcome) => outcome,
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
                        &workflow.source_map,
                    )?;
                    return Err(error);
                }
            };
            let published_output_names = published_outcome
                .outputs
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            context.record_outcome(node.id.clone(), published_outcome);
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
                    &workflow.source_map,
                )?;
            }
            if !published_output_names.is_empty() {
                emit_event(
                    sink,
                    build_event(
                        context.run_id,
                        workflow.definition.id,
                        sequence,
                        Some(node.id.clone()),
                        None,
                        ExecutionEventKind::NodeOutputProduced,
                        Some(format!(
                            "已产生 {} 个公开值输出",
                            published_output_names.len()
                        )),
                        Some(ExecutionEventPayload::NodeOutputsProduced {
                            output_names: published_output_names,
                        }),
                    ),
                    &workflow.source_map,
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
                &workflow.source_map,
            )?;

            let next_edge = select_next_edge(prepared.as_ref(), node_id, &outgoing, context)?;
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
                    &workflow.source_map,
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
    context: &RunContext,
) -> Result<Option<&'edge WorkflowEdge>, RuntimeError> {
    if matches!(node.flow(), NodeFlow::End) {
        return Ok(None);
    }
    let selected_branch = node.select_branch(context)?;
    Ok(outgoing
        .get(node_id)
        .into_iter()
        .flatten()
        .find(|edge| edge.branch == selected_branch)
        .copied())
}

/// 把组件目录或展开错误转换成现有工作流校验报告。
fn component_validation_report(
    message: String,
    node_id: Option<String>,
) -> crate::ValidationReport {
    crate::ValidationReport {
        valid: false,
        issues: vec![crate::ValidationIssue {
            code: crate::ValidationIssueCode::ComponentExpansionFailed,
            message,
            node_id,
            edge_id: None,
        }],
    }
}
