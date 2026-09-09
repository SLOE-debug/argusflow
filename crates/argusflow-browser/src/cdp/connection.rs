//! 有界总在途额度与取消安全的 CDP 调用接口。
use super::{
    lifecycle::{Health, PageState},
    transport,
};
use crate::BrowserError as Failure;
use crate::{BrowserConfig, ConnectionState};
use argusflow_core::{FailureKind, Operation};
use serde_json::Value;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};

pub(crate) struct Request {
    pub(crate) document: Option<(Arc<PageState>, u64)>,
    pub(crate) session: Option<String>,
    pub(crate) method: &'static str,
    pub(crate) params: Value,
    pub(crate) effect: bool,
    pub(crate) operation: Operation,
    pub(crate) response: oneshot::Sender<Result<Value, Failure>>,
    pub(crate) _permit: OwnedSemaphorePermit,
}
pub(crate) struct Inner {
    pub(crate) sender: mpsc::Sender<Request>,
    pub(crate) health: Arc<Health>,
    permits: Arc<Semaphore>,
    pub(crate) resources: Arc<Semaphore>,
    pub(crate) input: Arc<Semaphore>,
    pub(crate) stop: watch::Sender<bool>,
    pub(crate) finished: watch::Receiver<bool>,
    pub(crate) config: BrowserConfig,
}
impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}
#[derive(Clone)]
pub(crate) struct Connection {
    pub(crate) inner: Arc<Inner>,
}
impl Connection {
    pub(crate) async fn connect(
        url: &str,
        config: BrowserConfig,
        operation: &Operation,
    ) -> Result<Self, Failure> {
        let (sender, receiver) = mpsc::channel(config.max_in_flight);
        let (stop, stop_receiver) = watch::channel(false);
        let (finished_sender, finished) = watch::channel(false);
        let health = Arc::new(Health::default());
        let socket = transport::connect(url, &config, operation).await?;
        tokio::spawn(transport::run(
            socket,
            receiver,
            health.clone(),
            stop_receiver,
            finished_sender,
            config.clone(),
        ));
        Ok(Self {
            inner: Arc::new(Inner {
                sender,
                health,
                permits: Arc::new(Semaphore::new(config.max_in_flight)),
                resources: Arc::new(Semaphore::new(config.max_in_flight)),
                input: Arc::new(Semaphore::new(1)),
                stop,
                finished,
                config,
            }),
        })
    }
    pub(crate) fn state(&self) -> ConnectionState {
        if self.inner.health.disconnected.load(Ordering::Acquire) {
            ConnectionState::Disconnected
        } else {
            ConnectionState::Connected
        }
    }
    pub(crate) fn register(&self, session: String, target: String) -> Arc<PageState> {
        let state = Arc::new(PageState::new(target));
        self.inner
            .health
            .pages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(session, state.clone());
        state
    }
    pub(crate) async fn command(
        &self,
        session: Option<&str>,
        method: &'static str,
        params: Value,
        effect: bool,
        operation: &Operation,
    ) -> Result<Value, Failure> {
        operation.check("cdp_dispatch")?;
        self.inner.health.check(session)?;
        if *self.inner.stop.borrow() {
            return Err(Failure::new(
                FailureKind::Closed,
                "cdp_dispatch",
                "CDP 正在关闭",
            ));
        }
        let permit = self
            .inner
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                Failure::new(
                    FailureKind::Busy,
                    "cdp_queue",
                    "CDP 排队和待响应请求额度已满",
                )
            })?;
        self.dispatch(session, method, params, effect, operation, permit)
            .await
            .map_err(|error| operation.contextualize(error))
    }
    pub(crate) async fn command_wait(
        &self,
        session: Option<&str>,
        method: &'static str,
        params: Value,
        operation: &Operation,
    ) -> Result<Value, Failure> {
        let permit = tokio::time::timeout(
            operation.remaining(),
            self.inner.permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| Failure::new(FailureKind::Timeout, "cdp_cleanup", "等待清理请求额度超时"))?
        .map_err(|_| Failure::new(FailureKind::Closed, "cdp_cleanup", "清理请求额度已关闭"))?;
        self.dispatch(session, method, params, true, operation, permit)
            .await
    }
    async fn dispatch(
        &self,
        session: Option<&str>,
        method: &'static str,
        params: Value,
        effect: bool,
        operation: &Operation,
        permit: OwnedSemaphorePermit,
    ) -> Result<Value, Failure> {
        operation.check("cdp_dispatch")?;
        self.inner.health.check(session)?;
        let (sender, receiver) = oneshot::channel();
        let document = session
            .and_then(|session| {
                self.inner
                    .health
                    .pages
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(session)
                    .cloned()
            })
            .filter(|_| {
                if matches!(params["type"].as_str(), Some("keyUp" | "mouseReleased")) {
                    return false;
                }
                matches!(
                    method,
                    "DOM.querySelectorAll"
                        | "DOM.resolveNode"
                        | "DOM.focus"
                        | "DOM.scrollIntoViewIfNeeded"
                        | "Runtime.callFunctionOn"
                        | "Input.dispatchMouseEvent"
                        | "Input.dispatchKeyEvent"
                        | "Input.insertText"
                )
            })
            .map(|state| {
                let epoch = state.epoch.load(Ordering::Acquire);
                (state, epoch)
            });
        self.inner
            .sender
            .try_send(Request {
                document,
                session: session.map(str::to_owned),
                method,
                params,
                effect,
                operation: operation.clone(),
                response: sender,
                _permit: permit,
            })
            .map_err(|_| Failure::new(FailureKind::Closed, "cdp_queue", "CDP 请求队列关闭"))?;
        let result = match tokio::time::timeout(operation.remaining(), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Failure::new(
                FailureKind::Unavailable,
                "cdp_response",
                "CDP 连接未返回响应",
            )),
            Err(_) => Err(Failure::new(
                FailureKind::Timeout,
                "cdp_response",
                "CDP 总截止时间已到",
            )),
        };
        if result.is_ok() {
            operation.check("cdp_response")?;
        }
        result.map_err(|failure| {
            operation.contextualize(failure.with_resource(session.unwrap_or("browser")))
        })
    }
    pub(crate) async fn shutdown(&self, timeout: Duration) -> Result<(), Failure> {
        let _ = self.inner.stop.send(true);
        let mut finished = self.inner.finished.clone();
        tokio::time::timeout(timeout, async {
            while !*finished.borrow_and_update() {
                finished.changed().await.map_err(|_| {
                    Failure::new(
                        FailureKind::Unavailable,
                        "cdp_shutdown",
                        "CDP actor 意外退出",
                    )
                })?;
            }
            Ok(())
        })
        .await
        .map_err(|_| Failure::new(FailureKind::Timeout, "cdp_shutdown", "CDP 连接关闭超时"))?
    }
}
