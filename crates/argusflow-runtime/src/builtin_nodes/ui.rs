use std::{collections::BTreeMap, sync::Arc};

use argusflow_core::{
    ActionExecutionOptions, ActionOutputContract, ActionOutputKey, AutomationAction,
    AutomationError, ExecutionEventKind, ExecutionEventPayload, NodeEnvelope, NodeTypeId,
    OutputContractError, ResourceTypeId, TargetLocator, TargetScope, TargetWaitPolicy,
    UiExecutionPolicy, UiOperation, UiPostcondition, WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, ActionDispatcher, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution,
    NodeFlow, NodeOutcome, NodeValidationContext, PreparedNode, ResourceAccessKey, ResourceInput,
    RunContext, RuntimeError, ValidationIssue, ValueInput, ValueTypeId,
};

#[path = "ui/resolution.rs"]
mod resolution;
#[path = "ui/validation.rs"]
mod validation;

use resolution::{resolve_execution_scope, resolve_postcondition, resolve_target};

/// UI 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UiPayloadV1 {
    /// 资源作用域、目标定位与动作语义。
    operation: UiOperation,
}

/// 当前 UI payload，把动作语义与节点执行预算明确分离。
///
/// v3 是 Studio 当前写出的规范版本。
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
            2 | 3 => {
                let payload =
                    serde_json::from_value::<CurrentUiPayload>(definition.payload.clone())
                        .map_err(|error| {
                            NodeCompileError::new(format!(
                                "payload does not match registered schema: {error}"
                            ))
                        })?;
                (payload.operation, payload.execution)
            }
            version => {
                return Err(NodeCompileError::new(format!(
                    "unsupported payload version {version}; expected 1, 2 or 3"
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
        if let Some(postcondition) = &self.execution.postcondition {
            match postcondition {
                UiPostcondition::NewText {
                    query,
                    stable_context,
                } => {
                    inputs.push(ValueInput::text(&query.text));
                    inputs.extend(
                        stable_context
                            .iter()
                            .map(|query| ValueInput::text(&query.text)),
                    );
                }
                UiPostcondition::TextPresent { query } => {
                    inputs.push(ValueInput::text(&query.text));
                }
            }
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

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        if name == "confirmed" && self.execution.postcondition.is_some() {
            return Some(ValueTypeId::json());
        }
        match (self.operation.output_contract(), name) {
            (ActionOutputContract::Text, key) if key == ActionOutputKey::Text.as_str() => {
                Some(ValueTypeId::text())
            }
            (ActionOutputContract::Value, key) if key == ActionOutputKey::Value.as_str() => {
                Some(ValueTypeId::text())
            }
            (ActionOutputContract::Item, key) if key == ActionOutputKey::Item.as_str() => {
                Some(ValueTypeId::json())
            }
            (ActionOutputContract::Items, key) if key == ActionOutputKey::Items.as_str() => {
                Some(ValueTypeId::json())
            }
            (ActionOutputContract::TextAndLinks, key) if key == ActionOutputKey::Text.as_str() => {
                Some(ValueTypeId::text())
            }
            (ActionOutputContract::TextAndLinks, key) if key == ActionOutputKey::Links.as_str() => {
                Some(ValueTypeId::json())
            }
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
        node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let (target, prepared_target) = resolve_target(self.operation.target(), context)?;
        let postcondition = resolve_postcondition(&self.execution.postcondition, context)?;
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
                    postcondition_wait: self.execution.postcondition_wait,
                    prepared_target: Some(prepared_target),
                    postcondition,
                    trace_context: Some(argusflow_core::RunTraceContext {
                        run_id: context.run_id,
                        node_id: node_id.to_owned(),
                    }),
                },
            )
            .await?;
        if let Err(error) = validate_action_outputs(
            &self.operation,
            self.execution.postcondition.is_some(),
            &action_outcome.outputs,
        ) {
            return Err(RuntimeError::Automation(AutomationError::BackendFailed {
                backend: action_outcome.backend,
                message: format!(
                    "UI action output contract mismatch: expected {:?}, got {:?}",
                    error.expected, error.actual
                ),
            }));
        }
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
    };
    UiExecutionPolicy {
        target_wait,
        postcondition_wait: TargetWaitPolicy::default(),
        postcondition: None,
    }
}

/// 校验普通动作输出，或校验已由视觉后置条件确认的输入动作输出。
fn validate_action_outputs(
    operation: &UiOperation,
    has_postcondition: bool,
    outputs: &BTreeMap<String, serde_json::Value>,
) -> Result<(), OutputContractError> {
    if !has_postcondition {
        return operation.output_contract().validate(outputs);
    }
    let mut expected = operation
        .output_contract()
        .keys()
        .iter()
        .map(|key| key.as_str().to_owned())
        .collect::<Vec<_>>();
    expected.push("confirmed".to_owned());
    expected.sort();
    let actual = outputs.keys().cloned().collect::<Vec<_>>();
    if actual == expected
        && outputs
            .get("confirmed")
            .is_some_and(serde_json::Value::is_boolean)
    {
        return Ok(());
    }
    Err(OutputContractError { expected, actual })
}
