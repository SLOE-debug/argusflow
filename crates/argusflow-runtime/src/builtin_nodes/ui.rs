use std::sync::Arc;

use argusflow_core::{
    ActionExecutionOptions, AutomationAction, ExecutionEventKind, ExecutionEventPayload,
    NodeEnvelope, NodeTypeId, ResourceTypeId, TargetLocator, TargetScope, UiExecutionPolicy,
    UiOperation, WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, ActionDispatcher, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution,
    NodeFlow, NodeOutcome, NodeValidationContext, PreparedNode, ResourceAccessKey, ResourceInput,
    RunContext, RuntimeError, ValidationIssue, ValueInput,
};

#[path = "ui/resolution.rs"]
mod resolution;
#[path = "ui/validation.rs"]
mod validation;

pub(super) use resolution::resolve_execution_scope;
use resolution::resolve_target;

/// 当前 UI payload，把动作语义与节点执行预算明确分离。
///
/// v5 只承载写操作；读取、断言与分支统一由 Observe 节点表达。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentUiPayload {
    /// 资源作用域、目标定位与动作语义。
    operation: UiOperation,
    /// 当前节点为了完成动作可以使用的执行策略。
    execution: UiExecutionPolicy,
}

/// 创建持有 Planner dispatcher 的 UI 节点编译器。
pub(super) fn compiler(dispatcher: Arc<dyn ActionDispatcher>) -> Arc<dyn NodeCompiler> {
    Arc::new(UiCompiler {
        type_id: NodeTypeId::new("argus.ui"),
        dispatcher,
    })
}

/// 将动态 UI payload 冻结为强类型动作与 dispatcher 依赖。
struct UiCompiler {
    /// 注册表使用的稳定类型 ID。
    type_id: NodeTypeId,
    /// 执行阶段使用的已装配 Planner。
    dispatcher: Arc<dyn ActionDispatcher>,
}

impl NodeCompiler for UiCompiler {
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        if definition.version != 5 {
            return Err(NodeCompileError::new(format!(
                "界面操作的设置版本为 {}，当前只支持版本 5",
                definition.version
            )));
        }
        let payload = serde_json::from_value::<CurrentUiPayload>(definition.payload.clone())
            .map_err(|error| NodeCompileError::new(format!("界面操作的设置格式不正确：{error}")))?;
        let operation = payload.operation;
        let execution = payload.execution;
        let resource_type = match &operation.target().scope {
            TargetScope::Application { .. } => Some(ResourceTypeId::application()),
            TargetScope::Browser { .. } => Some(ResourceTypeId::browser()),
            TargetScope::Current => None,
        };
        Ok(Arc::new(UiNode {
            operation,
            execution,
            resource_type,
            dispatcher: Arc::clone(&self.dispatcher),
        }))
    }
}

/// 已解码并绑定 Planner dispatcher 的 UI 节点。
struct UiNode {
    /// 执行和依赖校验共享的语义操作。
    operation: UiOperation,
    /// 目标等待等节点级执行预算。
    execution: UiExecutionPolicy,
    /// 显式作用域要求的资源类型；Current 不需要资源。
    resource_type: Option<ResourceTypeId>,
    /// 已装配全部 ActionBackend 的动作分发器。
    dispatcher: Arc<dyn ActionDispatcher>,
}

impl std::fmt::Debug for UiNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UiNode")
            .field("operation", &self.operation)
            .field("execution", &self.execution)
            .field("resource_type", &self.resource_type)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl PreparedNode for UiNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        match &self.operation {
            UiOperation::Click { .. } => "UI Click",
            UiOperation::SetValue { .. } => "UI SetValue",
            UiOperation::PressKey { .. } => "UI PressKey",
            UiOperation::TypeText { .. } => "UI TypeText",
        }
        .to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        validation::validate_ui_node(&self.operation, &self.execution, context)
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        let mut inputs = Vec::new();
        match &self.operation {
            UiOperation::SetValue { value, .. } | UiOperation::TypeText { value, .. } => {
                inputs.push(ValueInput::text(value));
            }
            _ => {}
        }
        if let TargetLocator::Query { query } = &self.operation.target().locator {
            inputs.extend(query.bindings.values().map(ValueInput::text));
        }
        inputs
    }

    fn resource_inputs(&self) -> Vec<ResourceInput<'_>> {
        match (&self.operation.target().scope, self.resource_type.as_ref()) {
            (
                TargetScope::Application { resource } | TargetScope::Browser { resource },
                Some(kind),
            ) => {
                vec![ResourceInput {
                    reference: resource,
                    expected_type: kind,
                }]
            }
            _ => Vec::new(),
        }
    }

    fn access_set(&self, _node_id: &str, context: &RunContext) -> Result<AccessSet, RuntimeError> {
        let key = match &self.operation.target().scope {
            TargetScope::Current => ResourceAccessKey::global("ui.current"),
            TargetScope::Application { resource } | TargetScope::Browser { resource } => {
                context.resources().access_key(resource)?
            }
        };
        Ok(AccessSet::exclusive(key))
    }

    async fn execute(
        &self,
        node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let (target, prepared_target) = resolve_target(self.operation.target(), context)?;
        let scope = resolve_execution_scope(&target.scope, context)?;
        let action = match &self.operation {
            UiOperation::Click { .. } => AutomationAction::Click { target },
            UiOperation::SetValue { value, .. } => AutomationAction::SetValue {
                target,
                value: context.resolve_text(value)?,
            },
            UiOperation::PressKey { chord, .. } => AutomationAction::PressKey {
                target,
                chord: chord.clone(),
            },
            UiOperation::TypeText { value, .. } => AutomationAction::TypeText {
                target,
                value: context.resolve_text(value)?,
            },
        };
        let action_outcome = self
            .dispatcher
            .execute_with_options(
                &action,
                scope,
                ActionExecutionOptions {
                    target_wait: self.execution.target_wait,
                    prepared_target: Some(prepared_target),
                    trace_context: Some(argusflow_core::RunTraceContext {
                        run_id: context.run_id,
                        node_id: node_id.to_owned(),
                        node_sequence: context.current_node_sequence().ok_or_else(|| {
                            RuntimeError::ExecutionInvariant(
                                "UI 节点执行时缺少节点执行序号".to_owned(),
                            )
                        })?,
                    }),
                },
            )
            .await?;
        // 失败现场在 fallback 前产生，因此事件顺序也先于最终成功后端。
        let mut events = action_outcome
            .diagnostic_evidence
            .into_iter()
            .map(|evidence| NodeEvent {
                kind: ExecutionEventKind::DiagnosticEvidenceCaptured,
                message: Some("自动化失败现场已保存".to_owned()),
                payload: Some(ExecutionEventPayload::DiagnosticEvidenceCaptured {
                    evidence_id: evidence.evidence_id,
                    backend: evidence.backend,
                    branch_path: evidence.branch_path,
                    recovered_by_fallback: evidence.recovered_by_fallback,
                }),
            })
            .collect::<Vec<_>>();
        events.push(NodeEvent {
            kind: ExecutionEventKind::BackendSelected,
            message: Some(action_outcome.message),
            payload: Some(ExecutionEventPayload::BackendSelected {
                backend: action_outcome.backend,
            }),
        });
        Ok(NodeExecution {
            outcome: NodeOutcome::default(),
            events,
            ..NodeExecution::default()
        })
    }
}
