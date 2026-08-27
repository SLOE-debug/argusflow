use std::sync::Arc;

use argusflow_core::{
    BrowserOperation, BrowserSession, BrowserSessionProvider, NodeEnvelope, NodeTypeId,
    ResourceTypeId, WorkflowPermissions,
};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    AccessSet, NodeCompileError, NodeCompiler, NodeExecution, NodeFlow, NodeOutcome,
    NodeValidationContext, PreparedNode, ResourceInput, RunContext, RuntimeError, ValidationIssue,
    ValidationIssueCode, ValueInput,
};

/// Browser Operation 节点的强类型 payload。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserOperationPayload {
    /// 当前节点执行的浏览器语义动作。
    operation: BrowserOperation,
}

/// 创建持有浏览器会话提供器的操作节点编译器。
pub(super) fn compiler(provider: Arc<dyn BrowserSessionProvider>) -> Arc<dyn NodeCompiler> {
    Arc::new(BrowserOperationCompiler {
        type_id: NodeTypeId::new("argus.browser.operation"),
        provider,
    })
}

/// 解码浏览器操作并冻结资源提供器依赖。
struct BrowserOperationCompiler {
    /// 注册表使用的稳定类型 ID。
    type_id: NodeTypeId,
    /// 执行 Navigate 的浏览器会话提供器。
    provider: Arc<dyn BrowserSessionProvider>,
}

impl NodeCompiler for BrowserOperationCompiler {
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
        let payload = serde_json::from_value::<BrowserOperationPayload>(definition.payload.clone())
            .map_err(|error| {
                NodeCompileError::new(format!("payload does not match registered schema: {error}"))
            })?;
        Ok(Arc::new(BrowserOperationNode {
            operation: payload.operation,
            browser_type: ResourceTypeId::browser(),
            provider: Arc::clone(&self.provider),
        }))
    }
}

/// 已冻结的浏览器会话操作。
struct BrowserOperationNode {
    /// 资源引用和 URL 表达式。
    operation: BrowserOperation,
    /// Navigate 要求的资源端口类型。
    browser_type: ResourceTypeId,
    /// 负责实际 CDP 导航的提供器。
    provider: Arc<dyn BrowserSessionProvider>,
}

impl std::fmt::Debug for BrowserOperationNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserOperationNode")
            .field("operation", &self.operation)
            .field("browser_type", &self.browser_type)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl PreparedNode for BrowserOperationNode {
    fn flow(&self) -> NodeFlow {
        NodeFlow::Linear
    }

    fn label(&self) -> String {
        "Browser Navigate".to_owned()
    }

    fn validate(&self, context: &NodeValidationContext<'_>) -> Vec<ValidationIssue> {
        let BrowserOperation::Navigate { url, .. } = &self.operation;
        match url {
            argusflow_core::ValueExpr::Literal { value }
                if value.as_str().is_none_or(|url| !is_http_url(url)) =>
            {
                vec![context.issue(
                    ValidationIssueCode::InvalidBrowserSpec,
                    "导航地址字面量必须是绝对 HTTP(S) URL",
                )]
            }
            _ => Vec::new(),
        }
    }

    fn value_inputs(&self) -> Vec<ValueInput<'_>> {
        let BrowserOperation::Navigate { url, .. } = &self.operation;
        vec![ValueInput::text(url)]
    }

    fn resource_inputs(&self) -> Vec<ResourceInput<'_>> {
        let BrowserOperation::Navigate { browser, .. } = &self.operation;
        vec![ResourceInput {
            reference: browser,
            expected_type: &self.browser_type,
        }]
    }

    fn access_set(&self, _node_id: &str, context: &RunContext) -> Result<AccessSet, RuntimeError> {
        let BrowserOperation::Navigate { browser, .. } = &self.operation;
        let access_key = context.resources().access_key(browser)?;
        Ok(AccessSet::exclusive(access_key))
    }

    async fn execute(
        &self,
        _node_id: &str,
        _permissions: &WorkflowPermissions,
        context: &mut RunContext,
    ) -> Result<NodeExecution, RuntimeError> {
        let BrowserOperation::Navigate { browser, url } = &self.operation;
        let session = context
            .resources()
            .get::<BrowserSession>(browser, &self.browser_type)?
            .clone();
        let url = context.resolve_text(url)?;
        self.provider.navigate(&session, &url).await?;
        Ok(NodeExecution {
            outcome: NodeOutcome::default(),
            events: Vec::new(),
        })
    }
}

/// 判断导航目标是否是非空绝对 HTTP(S) URL。
fn is_http_url(value: &str) -> bool {
    value.split_once("://").is_some_and(|(scheme, remainder)| {
        matches!(scheme, "http" | "https") && !remainder.trim().is_empty()
    })
}
