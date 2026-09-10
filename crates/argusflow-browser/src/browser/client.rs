//! 浏览器资源所有权和页面装配。
use super::{BrowserConfig, LaunchOptions, endpoint};
use crate::BrowserError as Failure;
use crate::{
    ConnectionState, Page, PageInfo,
    cdp::{Cleanup, Connection, UnknownResource},
    process::{self, Managed},
};
use argusflow_core::{FailureKind, Operation, OperationOptions};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

struct Inner {
    connection: Connection,
    managed: Mutex<Option<Managed>>,
    cleanup: Mutex<()>,
    pages: Mutex<Vec<Page>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.connection.inner.health.disconnect();
        let _ = self.connection.inner.stop.send(true);
    }
}

/// 一个浏览器根连接；外部连接的 shutdown 不关闭浏览器。
#[derive(Clone)]
pub struct Browser {
    inner: Arc<Inner>,
}
impl Browser {
    /// 连接显式提供的调试端点。
    pub async fn connect(
        endpoint: &str,
        config: BrowserConfig,
        options: OperationOptions,
    ) -> Result<Self, Failure> {
        let operation = Operation::new(options);
        Self::connect_with_operation(endpoint, config, &operation).await
    }
    /// 连接端点，沿用调用方的截止时间、取消与副作用票据。
    pub async fn connect_with_operation(
        endpoint: &str,
        config: BrowserConfig,
        operation: &Operation,
    ) -> Result<Self, Failure> {
        config.validate()?;
        let mut guard = operation.cancel_on_drop();
        let url = endpoint::resolve(endpoint, operation, &config).await?;
        let connection = Connection::connect(&url, config, operation).await?;
        guard.disarm();
        Ok(Self {
            inner: Arc::new(Inner {
                connection,
                managed: Mutex::new(None),
                cleanup: Mutex::new(()),
                pages: Mutex::new(Vec::new()),
            }),
        })
    }
    /// 启动独立配置目录的 Chromium 并连接；失败时回收本次创建的资源。
    pub async fn launch(options: LaunchOptions, config: BrowserConfig) -> Result<Self, Failure> {
        let operation = Operation::new(OperationOptions::new(options.timeout)?);
        Self::launch_with_operation(options, config, &operation).await
    }
    /// 创建自有浏览器，使用外层总时限；取消时回收未交付的自有资源。
    pub async fn launch_with_operation(
        options: LaunchOptions,
        config: BrowserConfig,
        operation: &Operation,
    ) -> Result<Self, Failure> {
        config.validate()?;
        let mut guard = operation.cancel_on_drop();
        let profile_root = endpoint::profile_root();
        let permit = process::reserve()?;
        tokio::fs::create_dir_all(&profile_root)
            .await
            .map_err(|error| {
                Failure::new(
                    FailureKind::Unavailable,
                    "browser_profile",
                    "无法创建浏览器配置目录",
                )
                .with_source(error)
            })?;
        let profile = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(profile_root)
            .map_err(|error| {
                Failure::new(
                    FailureKind::Unavailable,
                    "browser_profile",
                    "无法建立独立配置目录",
                )
                .with_source(error)
            })?;
        operation.begin_effect("browser_launch")?;
        let child = endpoint::launch(&options, profile.path())?;
        let mut managed = Managed::new(child, profile, permit)?;
        let url = endpoint::wait_endpoint(
            managed.child.as_mut().expect("owned child"),
            managed.profile.as_ref().expect("owned profile").path(),
            operation,
        )
        .await?;
        let connection = Connection::connect(&url, config, operation).await?;
        guard.disarm();
        Ok(Self {
            inner: Arc::new(Inner {
                connection,
                managed: Mutex::new(Some(managed)),
                cleanup: Mutex::new(()),
                pages: Mutex::new(Vec::new()),
            }),
        })
    }
    /// 当前连接状态。
    pub fn state(&self) -> ConnectionState {
        self.inner.connection.state()
    }
    /// 列举普通 page targets，不猜测用户希望使用哪个页面。
    pub async fn pages(&self, options: OperationOptions) -> Result<Vec<PageInfo>, Failure> {
        let operation = Operation::new(options);
        self.pages_with_operation(&operation).await
    }
    /// 使用共享票据列举页面，不延长父级时限。
    pub async fn pages_with_operation(
        &self,
        operation: &Operation,
    ) -> Result<Vec<PageInfo>, Failure> {
        let mut guard = operation.cancel_on_drop();
        let result = self
            .inner
            .connection
            .command(None, "Target.getTargets", json!({}), false, operation)
            .await?;
        let targets = result["targetInfos"]
            .as_array()
            .ok_or_else(|| protocol("Target.getTargets 缺少 targetInfos"))?;
        let pages = targets
            .iter()
            .filter(|target| target["type"].as_str() == Some("page"))
            .map(|target| {
                serde_json::from_value(target.clone())
                    .map_err(|error| protocol("page target 格式错误").with_source(error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        guard.disarm();
        Ok(pages)
    }
    /// 附加明确指定的页面 ID；重复附加复用同一个页面会话。
    pub async fn attach(
        &self,
        target_id: &str,
        options: OperationOptions,
    ) -> Result<Page, Failure> {
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        self.attach_with_operation(target_id, &operation).await
    }
    /// 使用共享票据附加指定页面。
    pub async fn attach_with_operation(
        &self,
        target_id: &str,
        operation: &Operation,
    ) -> Result<Page, Failure> {
        let mut guard = operation.cancel_on_drop();
        operation.check("page_attach")?;
        if target_id.is_empty() || target_id.len() > 1024 {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "page_attach",
                "页面标识为空或超过 1024 字节",
            ));
        }
        let mut pages =
            self.inner.pages.try_lock().map_err(|_| {
                Failure::new(FailureKind::Busy, "page_attach", "正在装配另一个页面")
            })?;
        pages.retain(|page| page.is_open());
        if let Some(page) = pages.iter().find(|page| page.target_id() == target_id) {
            guard.disarm();
            return Ok(page.clone());
        }
        if pages.len() >= self.inner.connection.inner.config.max_pages {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "page_attach",
                "附加页面数量超限",
            ));
        }
        let page = Page::attach(self.inner.connection.clone(), target_id, operation).await?;
        pages.push(page.clone());
        guard.disarm();
        Ok(page)
    }
    /// 在独立、断连即释放的 context 中创建新页面并附加。
    pub async fn new_page(&self, url: &str, options: OperationOptions) -> Result<Page, Failure> {
        let operation = Operation::new(options);
        self.new_page_with_operation(url, &operation).await
    }
    /// 使用共享票据创建自有页面；失败时回收 context 与会话。
    pub async fn new_page_with_operation(
        &self,
        url: &str,
        operation: &Operation,
    ) -> Result<Page, Failure> {
        endpoint::validate_url(url)?;
        let mut guard = operation.cancel_on_drop();
        let mut pages =
            self.inner.pages.try_lock().map_err(|_| {
                Failure::new(FailureKind::Busy, "page_create", "正在装配另一个页面")
            })?;
        pages.retain(|page| page.is_open());
        if pages.len() >= self.inner.connection.inner.config.max_pages {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "page_create",
                "页面数量超限",
            ));
        }
        let connection = &self.inner.connection;
        let permit = connection
            .inner
            .resources
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::new(FailureKind::Busy, "page_create", "资源清理额度已满"))?;
        let mut unknown = UnknownResource(Some(connection.clone()), operation.clone());
        let context = connection
            .command(
                None,
                "Target.createBrowserContext",
                json!({"disposeOnDetach":true}),
                true,
                operation,
            )
            .await?;
        let context = context["browserContextId"]
            .as_str()
            .ok_or_else(|| protocol("创建 context 响应缺少标识"))?
            .to_owned();
        let mut cleanup = Cleanup::new(connection.clone(), None, permit);
        cleanup.commands.push((
            "Target.disposeBrowserContext",
            json!({"browserContextId":context}),
        ));
        unknown.0.take();
        let result = self
            .inner
            .connection
            .command(
                None,
                "Target.createTarget",
                json!({"url":url,"browserContextId":context}),
                true,
                operation,
            )
            .await?;
        let target = result["targetId"]
            .as_str()
            .ok_or_else(|| protocol("创建页面响应缺少 targetId"))?;
        let mut page = Page::attach(connection.clone(), target, operation).await?;
        Arc::get_mut(&mut page.inner)
            .expect("new page has one owner")
            .context = Some(context);
        pages.push(page.clone());
        cleanup.commands.clear();
        guard.disarm();
        Ok(page)
    }
    /// 关闭本次创建的浏览器或脱离外部浏览器，清理仅限自有资源。
    pub async fn shutdown(&self, options: OperationOptions) -> Result<(), Failure> {
        let operation = Operation::new(options);
        self.shutdown_with_operation(&operation).await
    }
    /// 使用调用方的清理预算关闭自建浏览器，或仅脱离外部浏览器。
    pub async fn shutdown_with_operation(&self, operation: &Operation) -> Result<(), Failure> {
        operation.check("browser_shutdown")?;
        let _cleanup = tokio::time::timeout(operation.remaining(), self.inner.cleanup.lock())
            .await
            .map_err(|_| {
                Failure::new(FailureKind::Timeout, "browser_shutdown", "等待关闭锁超时")
            })?;
        let mut managed = self.inner.managed.lock().await;
        if let Some(resource) = managed.as_mut() {
            // Browser.close 经常先断开连接再返回响应；以进程真正退出为完成依据。
            let graceful = operation.child(OperationOptions::new(
                operation
                    .remaining()
                    .min(Duration::from_secs(2))
                    .max(Duration::from_millis(1)),
            )?);
            let _ = self
                .inner
                .connection
                .command(None, "Browser.close", json!({}), true, &graceful)
                .await;
            resource.reap(operation).await?;
            self.inner
                .connection
                .shutdown(operation.remaining())
                .await?;
            managed.take();
        } else {
            // 关闭 WebSocket 自动脱离所有 flat session，外部 Browser 不接收 close。
            self.inner
                .connection
                .shutdown(operation.remaining())
                .await?;
        }
        Ok(())
    }
}
fn protocol(message: &str) -> Failure {
    Failure::new(FailureKind::Protocol, "browser", message)
}
