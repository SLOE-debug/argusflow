use argusflow_core::{AppSession, ApplicationError, ApplicationSessionProvider, ApplicationSpec};
use async_trait::async_trait;

/// 宿主没有装配平台应用能力时使用的显式失败实现。
#[derive(Debug, Default)]
pub struct UnavailableApplicationSessionProvider;

#[async_trait]
impl ApplicationSessionProvider for UnavailableApplicationSessionProvider {
    async fn acquire(&self, _spec: &ApplicationSpec) -> Result<AppSession, ApplicationError> {
        Err(ApplicationError::LaunchFailed {
            message: "no application session provider has been configured".to_owned(),
        })
    }

    async fn cleanup(&self, _session: &AppSession) -> Result<(), ApplicationError> {
        Ok(())
    }
}
