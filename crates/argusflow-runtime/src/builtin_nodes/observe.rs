//! AQL v3 原子观察节点的编译、校验与有界执行。

use std::{sync::Arc, time::Duration};

use argusflow_core::{
    BackendKind, ControlPortId, ExecutionEventKind, ExecutionEventPayload, NodeEnvelope,
    NodeTypeId, ObservationPolicy, ObservationRequest, ObservationResult, ObservationUnknownReason,
    ObservationValue, ObservationValueType, ObserveSpec, QueryValueType, ResourceTypeId,
    TargetScope, WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution, NodeFlow, NodeOutcome,
    NodeValidationContext, ObservationDispatcher, PreparedNode, ResourceAccessKey, ResourceInput,
    RunContext, RuntimeError, ValidationIssue, ValidationIssueCode, ValueInput, ValueTypeId,
};

/// Observe v1 节点 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservePayload {
    /// 观察作用域、AQL、后端策略与 Unknown 预算。
    observation: ObserveSpec,
}

/// 创建绑定观察路由器的编译器。
pub(super) fn compiler(dispatcher: Arc<dyn ObservationDispatcher>) -> Arc<dyn NodeCompiler> {
    Arc::new(ObserveCompiler {
        type_id: NodeTypeId::new("argus.observe"),
        dispatcher,
    })
}

/// 动态 payload 到冻结 AQL v3 观察节点的编译边界。
struct ObserveCompiler {
    /// 注册表使用的稳定类型 ID。
    type_id: NodeTypeId,
    /// 执行阶段使用的观察路由器。
    dispatcher: Arc<dyn ObservationDispatcher>,
}

impl NodeCompiler for ObserveCompiler {
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        if definition.version != 1 {
            return Err(NodeCompileError::new(format!(
                "检查界面的设置版本为 {}，当前只支持版本 1",
                definition.version,
            )));
        }
        let payload = serde_json::from_value::<ObservePayload>(definition.payload.clone())
            .map_err(|error| NodeCompileError::new(format!("检查界面的设置格式不正确：{error}")))?;
        let query = argusflow_query::parse_stored_observation(&payload.observation.query).map_err(
            |error| NodeCompileError::new(format!("AQL v3 observation is invalid: {error}")),
        )?;
        let parameter_types = argusflow_query::observation_parameter_types(&query)
            .map_err(|error| NodeCompileError::new(error.to_string()))?;
        let resource_type = match &payload.observation.scope {
            TargetScope::Application { .. } => Some(ResourceTypeId::application()),
            TargetScope::Browser { .. } => Some(ResourceTypeId::browser()),
            TargetScope::Current => None,
        };
        Ok(Arc::new(ObserveNode {
            spec: payload.observation,
            query,
            parameter_types,
            resource_type,
            dispatcher: Arc::clone(&self.dispatcher),
        }))
    }
}

/// 已解析 AQL 且绑定单次观察路由器的执行节点。
struct ObserveNode {
    /// 持久化业务配置。
    spec: ObserveSpec,
    /// 已完成语法和表达式类型检查的查询。
    query: argusflow_core::ObservationQuery,
    /// 动态绑定按源码上下文推导的类型。
    parameter_types: std::collections::BTreeMap<String, QueryValueType>,
    /// 显式作用域需要的资源类型。
    resource_type: Option<ResourceTypeId>,
    /// 单次事实源路由器。
    dispatcher: Arc<dyn ObservationDispatcher>,
}

impl std::fmt::Debug for ObserveNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObserveNode")
            .field("spec", &self.spec)
            .field("query", &self.query)
            .field("parameter_types", &self.parameter_types)
            .field("resource_type", &self.resource_type)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl PreparedNode for ObserveNode {
    fn flow(&self) -> NodeFlow {
        let ports = if self.query.value_type() == ObservationValueType::Boolean {
            vec![
                ControlPortId::new("true"),
                ControlPortId::new("false"),
                ControlPortId::new("unknown"),
            ]
        } else {
            vec![ControlPortId::new("known"), ControlPortId::new("unknown")]
        };
        NodeFlow::Branch { ports }
    }

    fn label(&self) -> String {
        "检查界面".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let referenced = self
            .parameter_types
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let bound = self.spec.query.bindings.keys().cloned().collect();
        if referenced != bound {
            issues.push(context.issue(
                ValidationIssueCode::InvalidAqlQuery,
                format!("检查规则使用的参数与已填写参数不一致：规则使用 {referenced:?}，当前填写 {bound:?}"),
            ));
        }
        if let ObservationPolicy::Bounded {
            timeout_ms,
            poll_interval_ms,
        } = self.spec.policy
            && (!(1..=600_000).contains(&timeout_ms) || !(1..=60_000).contains(&poll_interval_ms))
        {
            issues.push(context.issue(
                ValidationIssueCode::InvalidObservationPolicy,
                "最长等待时间应为 1 到 600000 毫秒，检查间隔应为 1 到 60000 毫秒",
            ));
        }
        validate_backend_scope(&self.spec, context, &mut issues);
        issues
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        self.spec
            .query
            .bindings
            .iter()
            .filter_map(|(name, expression)| {
                self.parameter_types.get(name).map(|value_type| {
                    let expected = match value_type {
                        QueryValueType::Text => ValueTypeId::text(),
                        QueryValueType::Integer => ValueTypeId::number(),
                        QueryValueType::Boolean => ValueTypeId::boolean(),
                    };
                    ValueInput::new(expression, expected)
                })
            })
            .collect()
    }

    fn resource_inputs(&self) -> Vec<ResourceInput<'_>> {
        match (&self.spec.scope, self.resource_type.as_ref()) {
            (
                TargetScope::Application { resource } | TargetScope::Browser { resource },
                Some(expected_type),
            ) => vec![ResourceInput {
                reference: resource,
                expected_type,
            }],
            _ => Vec::new(),
        }
    }

    fn value_output(&self, name: &str) -> Option<ValueTypeId> {
        (name == "result").then(ValueTypeId::json)
    }

    fn access_set(&self, _node_id: &str, context: &RunContext) -> Result<AccessSet, RuntimeError> {
        let key = match &self.spec.scope {
            TargetScope::Current => ResourceAccessKey::global("ui.current"),
            TargetScope::Application { resource } | TargetScope::Browser { resource } => {
                context.resources().access_key(resource)?
            }
        };
        Ok(AccessSet::read(key))
    }

    async fn execute(
        &self,
        node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let bindings = self
            .spec
            .query
            .bindings
            .iter()
            .map(|(name, expression)| {
                context
                    .resolve_value(expression)
                    .map(|value| (name.clone(), value))
            })
            .collect::<Result<_, _>>()?;
        let query = argusflow_query::resolve_observation_parameters(&self.query, &bindings)
            .map_err(|error| RuntimeError::NodeExecution {
                message: format!("无法准备界面检查规则：{error}"),
            })?;
        let scope = super::ui::resolve_execution_scope(&self.spec.scope, context)?;
        let request = ObservationRequest {
            scope: self.spec.scope.clone(),
            query,
            source: self.spec.query.source.clone(),
            backend_policy: self.spec.backend_policy.clone(),
        };
        let trace_context = argusflow_core::RunTraceContext {
            run_id: context.run_id,
            node_id: node_id.to_owned(),
            node_sequence: context.current_node_sequence().ok_or_else(|| {
                RuntimeError::ExecutionInvariant("检查节点执行时缺少节点执行序号".to_owned())
            })?,
        };
        let result = self
            .observe_with_policy(&request, scope, &trace_context)
            .await;
        let branch = branch_for(&result);
        let (backend, known) = match &result {
            ObservationResult::Known { backend, .. } => (Some(*backend), true),
            ObservationResult::Unknown { backend, .. } => (*backend, false),
        };
        let output =
            serde_json::to_value(&result).map_err(|error| RuntimeError::NodeExecution {
                message: format!("无法保存界面检查结果：{error}"),
            })?;
        Ok(NodeExecution {
            outcome: NodeOutcome::values(std::collections::BTreeMap::from([(
                "result".to_owned(),
                output,
            )])),
            events: vec![NodeEvent {
                kind: ExecutionEventKind::ObservationEvaluated,
                message: Some(if known {
                    "已获得检查结果".to_owned()
                } else {
                    "暂时无法判断".to_owned()
                }),
                payload: Some(ExecutionEventPayload::ObservationEvaluated {
                    value_type: self.query.value_type(),
                    backend,
                    known,
                }),
            }],
            branch: Some(branch),
            termination: None,
        })
    }
}

impl ObserveNode {
    /// 执行单次观察，或只在可重试 Unknown 上使用共享截止时间重试。
    async fn observe_with_policy(
        &self,
        request: &ObservationRequest,
        scope: argusflow_core::AutomationExecutionScope,
        trace_context: &argusflow_core::RunTraceContext,
    ) -> ObservationResult {
        let ObservationPolicy::Bounded {
            timeout_ms,
            poll_interval_ms,
        } = self.spec.policy
        else {
            return self
                .dispatcher
                .observe_with_options(
                    request,
                    scope,
                    argusflow_core::ObservationExecutionOptions {
                        trace_context: Some(trace_context.clone()),
                    },
                )
                .await;
        };
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let result = self
                .dispatcher
                .observe_with_options(
                    request,
                    scope.clone(),
                    argusflow_core::ObservationExecutionOptions {
                        trace_context: Some(trace_context.clone()),
                    },
                )
                .await;
            let backend = match result {
                known @ ObservationResult::Known { .. } => return known,
                unknown @ ObservationResult::Unknown {
                    retryable: false, ..
                } => return unknown,
                ObservationResult::Unknown { backend, .. } => backend,
            };
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return ObservationResult::Unknown {
                    backend,
                    reason: ObservationUnknownReason::Timeout,
                    retryable: false,
                };
            }
            tokio::time::sleep_until(std::cmp::min(
                deadline,
                now + Duration::from_millis(poll_interval_ms),
            ))
            .await;
        }
    }
}

/// 根据 AQL 结果类型和值选择 Observe 的唯一控制端口。
fn branch_for(result: &ObservationResult) -> ControlPortId {
    match result {
        ObservationResult::Known {
            value: ObservationValue::Boolean(true),
            ..
        } => ControlPortId::new("true"),
        ObservationResult::Known {
            value: ObservationValue::Boolean(false),
            ..
        } => ControlPortId::new("false"),
        ObservationResult::Known { .. } => ControlPortId::new("known"),
        ObservationResult::Unknown { .. } => ControlPortId::new("unknown"),
    }
}

/// 显式资源作用域必须允许其唯一事实源后端。
fn validate_backend_scope(
    spec: &ObserveSpec,
    context: &NodeValidationContext<'_>,
    issues: &mut Vec<ValidationIssue>,
) {
    let required = match spec.scope {
        TargetScope::Application { .. } => Some(BackendKind::WindowsUia),
        TargetScope::Browser { .. } => Some(BackendKind::BrowserCdp),
        TargetScope::Current => None,
    };
    if required.is_some_and(|backend| !spec.backend_policy.allows(backend)) {
        issues.push(context.issue(
            ValidationIssueCode::InvalidBackendPolicy,
            "当前检查方式不支持所选应用或浏览器，请更换检查方式",
        ));
    }
}
