use std::{any::Any, sync::Arc};

use argusflow_core::{
    AppSession, ApplicationSessionProvider, BrowserSession, BrowserSessionProvider,
};
use async_trait::async_trait;

use crate::{ResourceCleanup, RuntimeError};

/// 将应用会话提供器冻结为单个资源实例的清理策略。
pub(crate) struct ApplicationResourceCleanup {
    /// 获取该资源时使用的同一提供器。
    provider: Arc<dyn ApplicationSessionProvider>,
}

impl ApplicationResourceCleanup {
    /// 为新获取的应用会话创建清理策略。
    pub(crate) fn new(provider: Arc<dyn ApplicationSessionProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ResourceCleanup for ApplicationResourceCleanup {
    async fn cleanup(&self, value: &(dyn Any + Send + Sync)) -> Result<(), RuntimeError> {
        let session = value.downcast_ref::<AppSession>().ok_or_else(|| {
            RuntimeError::ExecutionInvariant(
                "application resource cleanup received a mismatched value".to_owned(),
            )
        })?;
        self.provider.cleanup(session).await.map_err(Into::into)
    }
}

/// 将浏览器会话提供器冻结为单个资源实例的清理策略。
pub(crate) struct BrowserResourceCleanup {
    /// 获取该资源时使用的同一提供器。
    provider: Arc<dyn BrowserSessionProvider>,
}

impl BrowserResourceCleanup {
    /// 为新获取的浏览器会话创建清理策略。
    pub(crate) fn new(provider: Arc<dyn BrowserSessionProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ResourceCleanup for BrowserResourceCleanup {
    async fn cleanup(&self, value: &(dyn Any + Send + Sync)) -> Result<(), RuntimeError> {
        let session = value.downcast_ref::<BrowserSession>().ok_or_else(|| {
            RuntimeError::ExecutionInvariant(
                "browser resource cleanup received a mismatched value".to_owned(),
            )
        })?;
        self.provider.cleanup(session).await.map_err(Into::into)
    }
}
