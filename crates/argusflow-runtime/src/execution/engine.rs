use std::{collections::HashSet, sync::Arc};

use argusflow_core::{
    ApplicationSessionProvider, BrowserSessionProvider, ExecutionEvent, ExecutionEventKind,
    FlowComponentDefinition, RunInputs, RunStarted,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    dispatcher::ActionDispatcher,
    execution_events::{build_event, restore_component_source},
    path_runner::execute_path,
    run_context::RunContext,
    run_inputs::validate_run_inputs,
    scheduler::ResourceScheduler,
};
use crate::{
    application::UnavailableApplicationSessionProvider,
    browser::UnavailableBrowserSessionProvider,
    builtin_nodes,
    component::{ComponentRegistry, expand_components},
    error::RuntimeError,
    node_registry::{NodeCompiler, NodeRegistryError, NodeTypeRegistry},
    validation::validator::{PreparedWorkflow, prepare_workflow},
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
    /// 可选的 Run Trace 持久化工厂；缺省构造保持纯 Runtime 行为。
    trace_store: Option<Arc<dyn crate::RunTraceStore>>,
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
            node_types: builtin_nodes::registry(
                dispatcher,
                Arc::new(super::dispatcher::UnavailableObservationDispatcher),
                applications,
                browsers,
            ),
            scheduler: ResourceScheduler::default(),
            trace_store: None,
        }
    }

    /// 创建同时装配动作、观察、桌面应用与浏览器能力的工作流引擎。
    pub fn with_dispatchers(
        dispatcher: Arc<dyn ActionDispatcher>,
        observations: Arc<dyn super::dispatcher::ObservationDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
        browsers: Arc<dyn BrowserSessionProvider>,
    ) -> Self {
        Self {
            active_runs: Mutex::new(HashSet::new()),
            node_types: builtin_nodes::registry(dispatcher, observations, applications, browsers),
            scheduler: ResourceScheduler::default(),
            trace_store: None,
        }
    }

    /// 创建内置节点运行时，并追加宿主提供的开放节点类型编译器。
    pub fn with_node_compilers(
        dispatcher: Arc<dyn ActionDispatcher>,
        applications: Arc<dyn ApplicationSessionProvider>,
        browsers: Arc<dyn BrowserSessionProvider>,
        compilers: impl IntoIterator<Item = Arc<dyn NodeCompiler>>,
    ) -> Result<Self, NodeRegistryError> {
        let mut node_types = builtin_nodes::registry(
            dispatcher,
            Arc::new(super::dispatcher::UnavailableObservationDispatcher),
            applications,
            browsers,
        );
        for compiler in compilers {
            node_types.register(compiler)?;
        }
        Ok(Self {
            active_runs: Mutex::new(HashSet::new()),
            node_types,
            scheduler: ResourceScheduler::default(),
            trace_store: None,
        })
    }

    /// 为后续运行装配持久化 Trace Store，不改变节点或自动化执行语义。
    pub fn with_trace_store(mut self, trace_store: Arc<dyn crate::RunTraceStore>) -> Self {
        self.trace_store = Some(trace_store);
        self
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
        let original_workflow = workflow.clone();
        let component_snapshot = components.clone();
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
        // Trace 创建失败不能阻止本来可以执行的自动化；没有目录时仅本次观测降级。
        let trace = self.trace_store.as_ref().and_then(|store| {
            store
                .start_run(
                    run_id,
                    &original_workflow,
                    &workflow.definition,
                    &component_snapshot,
                    &inputs,
                )
                .ok()
        });
        self.active_runs.lock().await.insert(run_id);
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let _ = engine.execute(run_id, workflow, inputs, sink, trace).await;
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
        trace: Option<Arc<dyn crate::RunTraceSession>>,
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
        emit_traced_event(
            &sink,
            trace.as_ref(),
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

        let execution = execute_path(
            &workflow,
            &sink,
            trace.as_ref(),
            &mut context,
            &mut sequence,
            &self.scheduler,
        )
        .await;
        let cleanup_access = context.resources().cleanup_access_set();
        let _cleanup_guard = self.scheduler.acquire(cleanup_access).await;
        let cleanup = context.resources().cleanup_all().await;
        let result = execution.and(cleanup);
        match result {
            Ok(()) => {
                let delivery = emit_traced_event(
                    &sink,
                    trace.as_ref(),
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
                );
                if let Some(trace) = &trace {
                    trace.finish(crate::RunStatus::Completed, None, None);
                }
                delivery
            }
            Err(error) => {
                emit_traced_event(
                    &sink,
                    trace.as_ref(),
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
                if let Some(trace) = &trace {
                    trace.finish(crate::RunStatus::Failed, None, Some(&error.to_string()));
                }
                Err(error)
            }
        }
    }
}

/// 先恢复组件来源，再将同一事件写入 best-effort Trace 和实时产品事件流。
pub(super) fn emit_traced_event(
    sink: &Arc<dyn ExecutionEventSink>,
    trace: Option<&Arc<dyn crate::RunTraceSession>>,
    mut event: ExecutionEvent,
    source_map: &crate::ComponentSourceMap,
) -> Result<(), RuntimeError> {
    restore_component_source(&mut event, source_map);
    if let Some(trace) = trace {
        trace.record_event(&event);
    }
    sink.emit(event).map_err(RuntimeError::EventSink)
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
            scope_id: None,
            structure_path: Vec::new(),
        }],
    }
}
