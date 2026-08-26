use std::{path::Path, sync::Arc};

use argusflow_core::{
    ApplicationSessionProvider, ApplicationSpec, ExecutionEventKind, ExecutionEventPayload,
    NodeEnvelope, NodeTypeId, ResourceRef, ResourceTypeId, WorkflowCapabilityId,
    WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, NodeCompileError, NodeCompiler, NodeEvent, NodeExecution, NodeFlow, NodeOutcome,
    NodeValidationContext, PreparedNode, ResourceAccessKey, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode, resource_cleanup::ApplicationResourceCleanup,
};

/// Application 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationPayload {
    /// 应用身份、获取策略和生命周期策略。
    spec: ApplicationSpec,
}

/// 创建持有平台应用提供器的节点编译器。
pub(super) fn compiler(provider: Arc<dyn ApplicationSessionProvider>) -> Arc<dyn NodeCompiler> {
    Arc::new(ApplicationCompiler {
        type_id: NodeTypeId::new("argus.application"),
        provider,
    })
}

/// 只负责 Application payload 解码并冻结提供器依赖。
struct ApplicationCompiler {
    /// 注册表使用的稳定类型 ID。
    type_id: NodeTypeId,
    /// 节点获取和资源清理共享的平台提供器。
    provider: Arc<dyn ApplicationSessionProvider>,
}

impl NodeCompiler for ApplicationCompiler {
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        if definition.version != 1 {
            return Err(NodeCompileError::new(format!(
                "unsupported payload version {}; expected 1",
                definition.version,
            )));
        }
        let payload = serde_json::from_value::<ApplicationPayload>(definition.payload.clone())
            .map_err(|error| {
                NodeCompileError::new(format!("payload does not match registered schema: {error}"))
            })?;
        Ok(Arc::new(ApplicationNode {
            spec: payload.spec,
            resource_type: ResourceTypeId::application(),
            provider: Arc::clone(&self.provider),
        }))
    }
}

/// 已解码并绑定平台提供器的应用资源节点。
struct ApplicationNode {
    /// 获取阶段使用的完整应用契约。
    spec: ApplicationSpec,
    /// `session` 输出端口的开放资源类型。
    resource_type: ResourceTypeId,
    /// 获取和清理同一资源实例的提供器。
    provider: Arc<dyn ApplicationSessionProvider>,
}

impl std::fmt::Debug for ApplicationNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApplicationNode")
            .field("spec", &self.spec)
            .field("resource_type", &self.resource_type)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl PreparedNode for ApplicationNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Application".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        if !Path::new(self.spec.executable_path.trim()).is_absolute() {
            issues.push(context.issue(
                ValidationIssueCode::InvalidApplicationSpec,
                "应用 EXE 必须使用绝对路径",
            ));
        }
        if self.spec.window_title.value().trim().is_empty() {
            issues.push(context.issue(
                ValidationIssueCode::InvalidApplicationSpec,
                "应用窗口标题匹配文本不能为空",
            ));
        }
        if !(100..=60_000).contains(&self.spec.launch_timeout_ms) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidApplicationSpec,
                "应用启动超时必须在 100 到 60000 毫秒之间",
            ));
        }
        if self.spec.acquire_policy.may_launch()
            && !context
                .workflow
                .permissions
                .allows(&WorkflowCapabilityId::application_launch())
        {
            issues.push(context.issue(
                ValidationIssueCode::ApplicationPermissionDenied,
                "工作流权限未授权 Application 节点启动应用",
            ));
        }
        issues
    }

    fn resource_output(&self, name: &str) -> Option<&ResourceTypeId> {
        (name == "session").then_some(&self.resource_type)
    }

    fn access_set(&self, _node_id: &str, _context: &RunContext) -> Result<AccessSet, RuntimeError> {
        Ok(AccessSet::exclusive(application_access_key(&self.spec)))
    }

    async fn execute(
        &self,
        node_id: &str,
        permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        if self.spec.acquire_policy.may_launch()
            && !permissions.allows(&WorkflowCapabilityId::application_launch())
        {
            return Err(RuntimeError::CapabilityDenied {
                capability: WorkflowCapabilityId::application_launch(),
            });
        }
        let session = self.provider.acquire(&self.spec).await?;
        let resource_id = session.id;
        let output_name = "session".to_owned();
        context.resources_mut().insert(
            ResourceRef {
                producer_node_id: node_id.to_owned(),
                output_name: output_name.clone(),
            },
            resource_id,
            self.resource_type.clone(),
            session,
            Arc::new(ApplicationResourceCleanup::new(Arc::clone(&self.provider))),
            application_access_key(&self.spec),
        );
        Ok(NodeExecution {
            outcome: NodeOutcome {
                outputs: Default::default(),
                resources: vec![output_name.clone()],
            },
            events: vec![NodeEvent {
                kind: ExecutionEventKind::ResourceAcquired,
                message: Some("应用会话已获取".to_owned()),
                payload: Some(ExecutionEventPayload::ResourceAcquired {
                    output_name,
                    resource_type: self.resource_type.as_str().to_owned(),
                }),
            }],
        })
    }
}

/// 在获取前为可能附加同一外部窗口的应用节点建立稳定冲突键。
fn application_access_key(spec: &ApplicationSpec) -> ResourceAccessKey {
    ResourceAccessKey::external(format!(
        "application:{}:{:?}",
        spec.executable_path.to_lowercase(),
        spec.window_title,
    ))
}
