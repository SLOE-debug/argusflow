use std::sync::Arc;

use argusflow_core::{
    ActionExecutionOptions, AppSession, AutomationAction, AutomationExecutionScope, BrowserSession,
    ExecutionEventKind, ExecutionEventPayload, ExtractCardinality, NodeEnvelope, NodeTypeId,
    ResourceTypeId, TargetLocator, TargetScope, TargetWaitPolicy, UiExecutionPolicy, UiOperation,
    WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, ActionDispatcher, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution,
    NodeFlow, NodeOutcome, NodeValidationContext, PreparedNode, ResourceAccessKey, ResourceInput,
    RunContext, RuntimeError, ValidationIssue, ValueInput, ValueTypeId,
};

#[path = "ui/validation.rs"]
mod validation;

/// UI 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiPayloadV1 {
    /// 资源作用域、目标定位与动作语义。
    operation: UiOperation,
}

/// UI 节点 v2 payload，把动作语义与节点执行预算明确分离。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiPayloadV2 {
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
        let (operation, execution) = match definition.version {
            1 => {
                let payload = serde_json::from_value::<UiPayloadV1>(definition.payload.clone())
                    .map_err(|error| {
                        NodeCompileError::new(format!(
                            "payload does not match registered schema: {error}"
                        ))
                    })?;
                let execution = legacy_execution_policy(payload.operation.target().locator.clone());
                (payload.operation, execution)
            }
            2 => {
                let payload = serde_json::from_value::<UiPayloadV2>(definition.payload.clone())
                    .map_err(|error| {
                        NodeCompileError::new(format!(
                            "payload does not match registered schema: {error}"
                        ))
                    })?;
                (payload.operation, payload.execution)
            }
            version => {
                return Err(NodeCompileError::new(format!(
                    "unsupported payload version {version}; expected 1 or 2"
                )));
            }
        };
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
            UiOperation::GetText { .. } => "UI GetText",
            UiOperation::GetValue { .. } => "UI GetValue",
            UiOperation::Extract { .. } => "UI Extract",
            UiOperation::CollectLinks { .. } => "UI CollectLinks",
        }
        .to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        validation::validate_ui_node(&self.operation, self.execution, context)
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        match &self.operation {
            UiOperation::SetValue { value, .. } | UiOperation::TypeText { value, .. } => {
                vec![ValueInput::text(value)]
            }
            _ => Vec::new(),
        }
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

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        match &self.operation {
            UiOperation::GetText { .. } if name == "text" => Some(ValueTypeId::text()),
            UiOperation::GetValue { .. } if name == "value" => Some(ValueTypeId::text()),
            UiOperation::Extract {
                cardinality: ExtractCardinality::One,
                ..
            } if name == "item" => Some(ValueTypeId::json()),
            UiOperation::Extract {
                cardinality: ExtractCardinality::Many,
                ..
            } if name == "items" => Some(ValueTypeId::json()),
            UiOperation::CollectLinks { .. } if name == "text" => Some(ValueTypeId::text()),
            UiOperation::CollectLinks { .. } if name == "links" => Some(ValueTypeId::json()),
            _ => None,
        }
    }

    fn access_set(&self, _node_id: &str, context: &RunContext) -> Result<AccessSet, RuntimeError> {
        let key = match &self.operation.target().scope {
            TargetScope::Current => ResourceAccessKey::global("ui.current"),
            TargetScope::Application { resource } | TargetScope::Browser { resource } => {
                context.resources().access_key(resource)?
            }
        };
        Ok(match &self.operation {
            UiOperation::GetText { .. }
            | UiOperation::GetValue { .. }
            | UiOperation::Extract { .. }
            | UiOperation::CollectLinks { .. } => AccessSet::read(key),
            UiOperation::Click { .. }
            | UiOperation::SetValue { .. }
            | UiOperation::PressKey { .. }
            | UiOperation::TypeText { .. } => AccessSet::exclusive(key),
        })
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let target = self.operation.target().clone();
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
            UiOperation::GetText { .. } => AutomationAction::GetText { target },
            UiOperation::GetValue { .. } => AutomationAction::GetValue { target },
            UiOperation::Extract {
                cardinality,
                fields,
                ..
            } => AutomationAction::Extract {
                target,
                cardinality: *cardinality,
                fields: fields.clone(),
            },
            UiOperation::CollectLinks { .. } => AutomationAction::CollectLinks { target },
        };
        let action_outcome = self
            .dispatcher
            .execute_with_options(
                &action,
                scope,
                ActionExecutionOptions {
                    target_wait: self.execution.target_wait,
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
            outcome: NodeOutcome::values(action_outcome.outputs),
            events,
        })
    }
}

/// v1 UI payload 沿用定位类别对应的新统一默认值，同时仍只 prepare 一次动作。
fn legacy_execution_policy(locator: TargetLocator) -> UiExecutionPolicy {
    let target_wait = match locator {
        TargetLocator::Coordinate { .. } => TargetWaitPolicy::none(),
        TargetLocator::Focused => TargetWaitPolicy::none(),
        TargetLocator::Query { .. } => TargetWaitPolicy::bounded(5_000, 100),
        TargetLocator::Visual { .. } => TargetWaitPolicy::bounded(5_000, 300),
    };
    UiExecutionPolicy { target_wait }
}

/// 把资源引用解析成不进入持久化定义的瞬时后端作用域。
fn resolve_execution_scope(
    scope: &TargetScope,
    context: &RunContext,
) -> Result<AutomationExecutionScope, RuntimeError> {
    match scope {
        TargetScope::Current => Ok(AutomationExecutionScope::Current),
        TargetScope::Application { resource } => {
            let session = context
                .resources()
                .get::<AppSession>(resource, &ResourceTypeId::application())?;
            let [window] = session.windows.as_slice() else {
                return Err(RuntimeError::ExecutionInvariant(format!(
                    "application resource '{}.{}' does not contain exactly one window",
                    resource.producer_node_id, resource.output_name,
                )));
            };
            Ok(AutomationExecutionScope::Window {
                handle: window.handle,
                process_id: window.process_id,
                capabilities: session.capabilities.clone(),
            })
        }
        TargetScope::Browser { resource } => {
            let session = context
                .resources()
                .get::<BrowserSession>(resource, &ResourceTypeId::browser())?;
            Ok(AutomationExecutionScope::Browser {
                session_id: session.id,
                target_id: session.target_id.clone(),
            })
        }
    }
}
