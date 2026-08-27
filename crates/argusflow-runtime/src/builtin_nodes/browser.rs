use std::{path::Path, sync::Arc};

use argusflow_core::{
    AcquireBrowserSpec, BrowserAcquireMode, BrowserCleanupPolicy, BrowserSessionProvider,
    BrowserSpec, ExecutionEventKind, ExecutionEventPayload, NodeEnvelope, NodeTypeId, ResourceRef,
    ResourceTypeId, WorkflowCapabilityId, WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    NodeCompileError, NodeCompiler, NodeEvent, NodeExecution, NodeFlow, NodeOutcome,
    NodeValidationContext, PreparedNode, ResourceAccessKey, RunContext, RuntimeError,
    ValidationIssue, ValidationIssueCode, resource_cleanup::BrowserResourceCleanup,
};

/// Browser 节点的强类型 payload。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserPayloadV1 {
    /// 浏览器可执行文件、初始 URL 和启动边界。
    spec: BrowserSpec,
}

/// Browser v2 只获取会话资源，不隐式执行页面导航。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserPayloadV2 {
    /// 浏览器可执行文件、获取方式与生命周期边界。
    spec: AcquireBrowserSpec,
}

/// 创建持有浏览器会话提供器的节点编译器。
pub(super) fn compiler(provider: Arc<dyn BrowserSessionProvider>) -> Arc<dyn NodeCompiler> {
    Arc::new(BrowserCompiler {
        type_id: NodeTypeId::new("argus.browser"),
        provider,
    })
}

/// 只负责 Browser payload 解码并冻结提供器依赖。
struct BrowserCompiler {
    /// 注册表使用的稳定类型 ID。
    type_id: NodeTypeId,
    /// 节点获取和资源清理共享的浏览器提供器。
    provider: Arc<dyn BrowserSessionProvider>,
}

impl NodeCompiler for BrowserCompiler {
    fn type_id(&self) -> &NodeTypeId {
        &self.type_id
    }

    fn compile(
        &self,
        definition: &NodeEnvelope,
    ) -> Result<Arc<dyn PreparedNode>, NodeCompileError> {
        let (spec, legacy_initial_url) = match definition.version {
            1 => {
                let payload =
                    serde_json::from_value::<BrowserPayloadV1>(definition.payload.clone())
                        .map_err(payload_error)?;
                (
                    AcquireBrowserSpec {
                        executable_path: payload.spec.executable_path,
                        acquire_mode: BrowserAcquireMode::LaunchIsolatedCdp,
                        launch_timeout_ms: payload.spec.launch_timeout_ms,
                        cleanup_policy: BrowserCleanupPolicy::CloseOnWorkflowEnd,
                    },
                    Some(payload.spec.initial_url),
                )
            }
            2 => {
                let payload =
                    serde_json::from_value::<BrowserPayloadV2>(definition.payload.clone())
                        .map_err(payload_error)?;
                (payload.spec, None)
            }
            version => {
                return Err(NodeCompileError::new(format!(
                    "unsupported payload version {version}; expected 1 or 2",
                )));
            }
        };
        Ok(Arc::new(BrowserNode {
            spec,
            legacy_initial_url,
            resource_type: ResourceTypeId::browser(),
            provider: Arc::clone(&self.provider),
        }))
    }
}

/// 已解码并绑定浏览器提供器的资源节点。
struct BrowserNode {
    /// 获取阶段使用的完整浏览器契约。
    spec: AcquireBrowserSpec,
    /// v1 节点在获取后执行的兼容导航；v2 必须使用独立 Navigate 节点。
    legacy_initial_url: Option<String>,
    /// `session` 输出端口的开放资源类型。
    resource_type: ResourceTypeId,
    /// 获取和清理同一资源实例的提供器。
    provider: Arc<dyn BrowserSessionProvider>,
}

impl std::fmt::Debug for BrowserNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserNode")
            .field("spec", &self.spec)
            .field("legacy_initial_url", &self.legacy_initial_url)
            .field("resource_type", &self.resource_type)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl PreparedNode for BrowserNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Browser".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        if !Path::new(self.spec.executable_path.trim()).is_absolute() {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBrowserSpec,
                "浏览器 EXE 必须使用绝对路径",
            ));
        }
        if self
            .legacy_initial_url
            .as_deref()
            .is_some_and(|url| !is_http_url(url))
        {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBrowserSpec,
                "Browser v1 初始地址必须是绝对 HTTP(S) URL",
            ));
        }
        if !(100..=60_000).contains(&self.spec.launch_timeout_ms) {
            issues.push(context.issue(
                ValidationIssueCode::InvalidBrowserSpec,
                "浏览器启动超时必须在 100 到 60000 毫秒之间",
            ));
        }
        if !context
            .workflow
            .permissions
            .allows(&WorkflowCapabilityId::application_launch())
        {
            issues.push(context.issue(
                ValidationIssueCode::ApplicationPermissionDenied,
                "工作流权限未授权 Browser 节点启动浏览器",
            ));
        }
        issues
    }

    fn resource_output(&self, name: &str) -> Option<&ResourceTypeId> {
        (name == "session").then_some(&self.resource_type)
    }

    async fn execute(
        &self,
        node_id: &str,
        permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        if !permissions.allows(&WorkflowCapabilityId::application_launch()) {
            return Err(RuntimeError::CapabilityDenied {
                capability: WorkflowCapabilityId::application_launch(),
            });
        }
        let session = self.provider.acquire(&self.spec).await?;
        if let Some(url) = &self.legacy_initial_url
            && let Err(error) = self.provider.navigate(&session, url).await
        {
            let _ = self.provider.cleanup(&session).await;
            return Err(error.into());
        }
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
            Arc::new(BrowserResourceCleanup::new(Arc::clone(&self.provider))),
            ResourceAccessKey::Runtime(resource_id),
        );
        Ok(NodeExecution {
            outcome: NodeOutcome {
                outputs: Default::default(),
                resources: vec![output_name.clone()],
            },
            events: vec![NodeEvent {
                kind: ExecutionEventKind::ResourceAcquired,
                message: Some("浏览器 CDP 会话已获取".to_owned()),
                payload: Some(ExecutionEventPayload::ResourceAcquired {
                    output_name,
                    resource_type: self.resource_type.as_str().to_owned(),
                }),
            }],
        })
    }
}

/// 把 serde payload 错误统一收敛到节点编译边界。
fn payload_error(error: serde_json::Error) -> NodeCompileError {
    NodeCompileError::new(format!("payload does not match registered schema: {error}",))
}

/// 判断 Browser v1 兼容导航是否是绝对 HTTP(S) 地址。
fn is_http_url(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, remainder)| {
        matches!(scheme, "http" | "https") && !remainder.trim().is_empty()
    })
}
