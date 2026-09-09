//! 连接和页面代际只读状态，协议事件立即使旧元素失效。
use crate::BrowserError as Failure;
use argusflow_core::FailureKind;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// CDP 连接的只读生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 可收发请求。
    Connected,
    /// 已断开；旧 session 不能继续使用。
    Disconnected,
}
pub(crate) struct PageState {
    pub(crate) epoch: AtomicU64,
    pub(crate) closed: AtomicBool,
    pub(crate) target: String,
    pub(crate) main_frame: Mutex<Option<String>>,
}
impl PageState {
    pub(crate) fn new(target: String) -> Self {
        Self {
            epoch: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            target,
            main_frame: Mutex::new(None),
        }
    }
    pub(crate) fn check(&self) -> Result<(), Failure> {
        if self.closed.load(Ordering::Acquire) {
            Err(Failure::new(
                FailureKind::StaleHandle,
                "page",
                "页面或会话已关闭",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
pub(crate) struct Health {
    pub(crate) disconnected: AtomicBool,
    pub(crate) pages: Mutex<HashMap<String, Arc<PageState>>>,
}
impl Health {
    pub(crate) fn check(&self, session: Option<&str>) -> Result<(), Failure> {
        if self.disconnected.load(Ordering::Acquire) {
            return Err(Failure::new(
                FailureKind::Unavailable,
                "cdp_connection",
                "CDP 连接已经断开",
            ));
        }
        if let Some(session) = session {
            self.pages
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(session)
                .ok_or_else(|| {
                    Failure::new(
                        FailureKind::StaleHandle,
                        "cdp_session",
                        "CDP session 不存在",
                    )
                })?
                .check()?;
        }
        Ok(())
    }
    pub(crate) fn disconnect(&self) {
        self.disconnected.store(true, Ordering::Release);
        for page in self
            .pages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
        {
            page.closed.store(true, Ordering::Release);
        }
    }
    pub(crate) fn event(&self, message: &Value) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        let params = &message["params"];
        let mut pages = self.pages.lock().unwrap_or_else(|p| p.into_inner());
        match method {
            "Target.detachedFromTarget" => {
                if let Some(page) = params["sessionId"].as_str().and_then(|id| pages.get(id)) {
                    page.closed.store(true, Ordering::Release);
                }
            }
            "Target.targetDestroyed" | "Target.targetCrashed" => {
                for page in pages.values() {
                    if params["targetId"].as_str() == Some(page.target.as_str()) {
                        page.closed.store(true, Ordering::Release);
                    }
                }
            }
            "Inspector.detached" | "Inspector.targetCrashed" => {
                if let Some(page) = message["sessionId"].as_str().and_then(|id| pages.get(id)) {
                    page.closed.store(true, Ordering::Release);
                }
            }
            "Runtime.executionContextsCleared" | "DOM.documentUpdated" => {
                if let Some(page) = message["sessionId"].as_str().and_then(|id| pages.get(id)) {
                    page.epoch.fetch_add(1, Ordering::AcqRel);
                }
            }
            "Page.frameNavigated" => {
                if params["frame"].get("parentId").is_none()
                    && let Some(page) = message["sessionId"].as_str().and_then(|id| pages.get(id))
                {
                    page.epoch.fetch_add(1, Ordering::AcqRel);
                    *page.main_frame.lock().unwrap_or_else(|p| p.into_inner()) =
                        params["frame"]["id"].as_str().map(str::to_owned);
                }
            }
            _ => {}
        }
        pages.retain(|_, page| !page.closed.load(Ordering::Acquire));
    }
}
