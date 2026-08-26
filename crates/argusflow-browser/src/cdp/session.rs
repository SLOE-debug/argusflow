//! 浏览器连接和 page target session 的运行时注册表。

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use argusflow_core::ResourceId;
use serde_json::{Value, json};

use super::{failure::CdpProtocolError, protocol::CdpConnection};

/// 已附加到单个 page target 的持久会话。
#[derive(Debug)]
pub(crate) struct CdpPageSession {
    /// 与同一浏览器其它 target 共享的根连接。
    connection: CdpConnection,
    /// `Target.attachToTarget(flatten=true)` 返回的 session ID。
    session_id: String,
    /// 获取阶段冻结的 page target ID。
    target_id: String,
}

impl CdpPageSession {
    /// 附加 page target 并启用 Runtime/Page domain。
    pub(crate) async fn attach(
        connection: CdpConnection,
        target_id: String,
    ) -> Result<Arc<Self>, CdpProtocolError> {
        let attached = connection
            .command(
                None,
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
            )
            .await?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| CdpProtocolError::InvalidResponse {
                message: "Target.attachToTarget did not return sessionId".to_owned(),
            })?
            .to_owned();
        connection.register_session(session_id.clone(), target_id.clone());
        let page = Arc::new(Self {
            connection,
            session_id,
            target_id,
        });
        page.command("Runtime.enable", json!({})).await?;
        page.command("Page.enable", json!({})).await?;
        page.command("Inspector.enable", json!({})).await?;
        Ok(page)
    }

    /// 在当前 page session 上调用 CDP 方法。
    pub(crate) async fn command(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpProtocolError> {
        self.connection
            .command(Some(&self.session_id), method, params)
            .await
    }

    /// 返回冻结 target ID，供 scope 复验。
    pub(crate) fn target_id(&self) -> &str {
        &self.target_id
    }

    /// 请求 Chromium 主进程优雅关闭全部 target。
    pub(crate) async fn close_browser(&self) -> Result<(), CdpProtocolError> {
        self.connection
            .command(None, "Browser.close", json!({}))
            .await?;
        Ok(())
    }
}

/// 以 ResourceId 为键的同步只读快路径注册表。
#[derive(Debug, Default)]
pub(crate) struct CdpSessionRegistry {
    /// 锁只保护 Arc 的插入、获取和移除，不跨 await 持有。
    sessions: RwLock<HashMap<ResourceId, Arc<CdpPageSession>>>,
}

impl CdpSessionRegistry {
    /// 注册新获取的浏览器资源。
    pub(crate) fn insert(&self, resource_id: ResourceId, session: Arc<CdpPageSession>) {
        self.sessions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(resource_id, session);
    }

    /// 返回当前仍由 runtime 管理的页面会话。
    pub(crate) fn get(&self, resource_id: ResourceId) -> Option<Arc<CdpPageSession>> {
        self.sessions
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&resource_id)
            .cloned()
    }

    /// 移除即将清理的页面会话。
    pub(crate) fn remove(&self, resource_id: ResourceId) -> Option<Arc<CdpPageSession>> {
        self.sessions
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&resource_id)
    }
}
