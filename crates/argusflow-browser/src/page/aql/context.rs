//! 所有文档根持有代际证明，跨 frame 结果同时校验祖先。
use super::super::handle::{protocol, remote_value, stale};
use super::{boundary::FrameSession, model::BrowserMatch};
use crate::{BrowserError, Page, cdp::Cleanup};
use argusflow_core::{FailureKind, Operation};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_GROUP: AtomicU64 = AtomicU64::new(1);

pub(super) struct DocumentContext {
    pub page: Page,
    pub epoch: u64,
    pub frame_id: String,
    pub owner: Option<BrowserMatch>,
    pub _lease: Option<Arc<FrameSession>>,
}
impl DocumentContext {
    pub fn check(&self) -> Result<(), BrowserError> {
        self.page.inner.state.check()?;
        if self.epoch != self.page.inner.state.epoch.load(Ordering::Acquire) {
            return Err(stale());
        }
        if let Some(owner) = &self.owner {
            owner.context.check()?;
        }
        Ok(())
    }
    pub fn top_page(&self) -> &Page {
        if let Some(owner) = &self.owner {
            owner.context.top_page()
        } else {
            &self.page
        }
    }
    pub async fn call(
        &self,
        backend_node: i64,
        function: &str,
        arguments: Vec<Value>,
        effect: bool,
        operation: &Operation,
    ) -> Result<Value, BrowserError> {
        self.check()?;
        let connection = &self.page.inner.connection;
        let permit = connection
            .inner
            .resources
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                BrowserError::new(FailureKind::Busy, "aql_objects", "浏览器远端对象额度已满")
            })?;
        // 清理异步进行；同一 Operation 内每次调用必须拥有独立对象组。
        let id = NEXT_GROUP
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| {
                BrowserError::new(FailureKind::ResourceLimit, "aql_objects", "对象组标识耗尽")
            })?;
        let group = format!("aql-{}-{id}", operation.id());
        let mut cleanup = Cleanup::new(
            connection.clone(),
            Some(self.page.inner.session.clone()),
            permit,
        );
        cleanup
            .commands
            .push(("Runtime.releaseObjectGroup", json!({"objectGroup":group})));
        let resolved = self
            .page
            .command(
                "DOM.resolveNode",
                json!({"backendNodeId":backend_node,"objectGroup":group}),
                false,
                operation,
            )
            .await?;
        let object = resolved["object"]["objectId"]
            .as_str()
            .ok_or_else(|| protocol("DOM 节点无法解析为远端对象"))?;
        self.check()?;
        let result = self.page.command("Runtime.callFunctionOn", json!({
            "objectId":object,"functionDeclaration":function,"arguments":arguments.into_iter().map(|value| json!({"value":value})).collect::<Vec<_>>(),
            "returnByValue":true,"silent":true
        }), effect, operation).await?;
        self.check()?;
        remote_value(result)
    }
}
