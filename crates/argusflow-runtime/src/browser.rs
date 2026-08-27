use argusflow_core::{AcquireBrowserSpec, BrowserError, BrowserSession, BrowserSessionProvider};
use async_trait::async_trait;

/// 宿主没有装配浏览器能力时使用的显式失败实现。
#[derive(Debug, Default)]
pub struct UnavailableBrowserSessionProvider;

#[async_trait]
impl BrowserSessionProvider for UnavailableBrowserSessionProvider {
    async fn acquire(&self, _spec: &AcquireBrowserSpec) -> Result<BrowserSession, BrowserError> {
        Err(BrowserError::LaunchFailed {
            message: "no browser session provider has been configured".to_owned(),
        })
    }

    async fn navigate(&self, _session: &BrowserSession, _url: &str) -> Result<(), BrowserError> {
        Err(BrowserError::NavigationFailed {
            message: "no browser session provider has been configured".to_owned(),
        })
    }

    async fn cleanup(&self, _session: &BrowserSession) -> Result<(), BrowserError> {
        Ok(())
    }
}
