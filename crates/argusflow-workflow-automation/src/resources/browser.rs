//! 浏览器与页面分别保留附加和自建的关闭语义。
use super::page_claim::{Claims, PageClaim};
use argusflow_browser::{Browser, Page};
use argusflow_core::Operation;
use argusflow_runtime::{Resource, RunError, TaskFuture};
use std::{
    any::Any,
    sync::atomic::{AtomicBool, Ordering},
};

/// 浏览器连接的作用域资源；Browser 本身区分自建与外部连接。
pub struct BrowserResource {
    pub(crate) browser: Browser,
    claims: Claims,
}
impl BrowserResource {
    /// 包装宿主持有的连接；用作 RunInputs 借用资源时不会被运行自动关闭。
    pub fn new(browser: Browser) -> Self {
        Self {
            browser,
            claims: Claims::default(),
        }
    }
    pub(crate) async fn attach(
        &self,
        target: &str,
        operation: &Operation,
    ) -> Result<PageResource, RunError> {
        let claim = PageClaim::acquire(&self.claims, target)?;
        let page = self
            .browser
            .attach_with_operation(target, operation)
            .await
            .map_err(|e| RunError::from(argusflow_core::Failure::from(e)))?;
        let mut resource = PageResource::attached(page);
        resource._claim = Some(claim);
        Ok(resource)
    }
    pub(crate) async fn new_page(
        &self,
        url: &str,
        operation: &Operation,
    ) -> Result<PageResource, RunError> {
        let page = self
            .browser
            .new_page_with_operation(url, operation)
            .await
            .map_err(|e| RunError::from(argusflow_core::Failure::from(e)))?;
        let claim = PageClaim::acquire(&self.claims, page.target_id())?;
        let mut resource = PageResource::owned(page);
        resource._claim = Some(claim);
        Ok(resource)
    }
}
impl Resource for BrowserResource {
    fn resource_type(&self) -> &str {
        super::BROWSER
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async move {
            self.browser
                .shutdown_with_operation(operation)
                .await
                .map_err(|e| RunError::from(argusflow_core::Failure::from(e)))
        })
    }
}
/// 页面资源；自建页面关闭，附加页面只脱离会话。
pub struct PageResource {
    pub(crate) page: Page,
    owned: bool,
    closed: AtomicBool,
    _claim: Option<PageClaim>,
}
impl PageResource {
    /// 包装一个外部已有页面，不取得关闭页面权限。
    pub fn attached(page: Page) -> Self {
        Self {
            page,
            owned: false,
            closed: AtomicBool::new(false),
            _claim: None,
        }
    }
    pub(crate) fn owned(page: Page) -> Self {
        Self {
            page,
            owned: true,
            closed: AtomicBool::new(false),
            _claim: None,
        }
    }
}
impl Resource for PageResource {
    fn resource_type(&self) -> &str {
        super::PAGE
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn cleanup<'a>(&'a self, operation: &'a Operation) -> TaskFuture<'a, ()> {
        Box::pin(async move {
            if self.closed.load(Ordering::Acquire) {
                return Ok(());
            }
            if !self.owned && !self.page.is_open() {
                self.closed.store(true, Ordering::Release);
                return Ok(());
            }
            let result = if self.owned {
                self.page.close_with_operation(operation).await
            } else {
                self.page.detach_with_operation(operation).await
            };
            result.map_err(|e| RunError::from(argusflow_core::Failure::from(e)))?;
            self.closed.store(true, Ordering::Release);
            Ok(())
        })
    }
}
