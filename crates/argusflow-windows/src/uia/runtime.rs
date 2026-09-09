//! UIA 异步调用句柄，不在 Tokio 线程中执行 COM。
use super::worker::{self, Request, Response};
use crate::WindowsError as Failure;
use crate::{ElementHandle, ElementSnapshot, Query, UiaAction, UiaConfig};
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{SyncSender, TrySendError},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::{Notify, oneshot};

static NEXT_RUNTIME: AtomicU64 = AtomicU64::new(1);

/// UIA 实例的只读生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaState {
    /// COM 正在初始化。
    Starting,
    /// 可接受请求。
    Ready,
    /// 请求已取消或超时，但原生调用尚未退出。
    Unresponsive,
    /// 正在停止，禁止新请求。
    Stopping,
    /// 线程和 COM 对象已释放。
    Stopped,
    /// 初始化或 worker 发生不可恢复错误。
    Failed,
}

pub(crate) struct Shared {
    pub(crate) state: Mutex<UiaState>,
    pub(crate) active: Mutex<Option<Operation>>,
    pub(crate) stopping: AtomicBool,
    pub(crate) finished: Notify,
}
impl Shared {
    pub(crate) fn state(&self) -> UiaState {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if self.stopping.load(Ordering::Acquire)
            && !matches!(state, UiaState::Stopped | UiaState::Failed)
        {
            return UiaState::Stopping;
        }
        if state == UiaState::Ready
            && self
                .active
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_some_and(|op| op.is_cancelled() || op.remaining().is_zero())
        {
            UiaState::Unresponsive
        } else {
            state
        }
    }
}

struct Inner {
    sender: SyncSender<Request>,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
    id: u64,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        if let Some(operation) = self
            .shared
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            operation.cancel();
        }
        if let Some(thread) = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
    }
}

/// 应用生命周期内复用的 UIA 实例；Clone 不创建新线程。
#[derive(Clone)]
pub struct UiaRuntime {
    inner: Arc<Inner>,
}

impl UiaRuntime {
    /// 启动专用 MTA 线程，等待初始化就绪；失败不会后台重建线程。
    pub async fn start(config: UiaConfig, options: OperationOptions) -> Result<Self, Failure> {
        if config.queue_capacity == 0
            || config.queue_capacity > 4096
            || config.max_results == 0
            || config.max_results > 4096
            || config.max_nodes == 0
            || config.max_nodes > 100_000
            || config.max_depth == 0
            || config.max_depth > 256
            || config.lease_duration.is_zero()
            || config.lease_duration > Duration::from_secs(3600)
            || [config.connection_timeout, config.transaction_timeout]
                .iter()
                .any(|value| value.is_zero() || value.as_millis() > u128::from(u32::MAX))
        {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "uia_config",
                "UIA 配置超出允许范围",
            ));
        }
        let operation = Operation::new(options);
        let _guard = operation.cancel_on_drop();
        let (sender, receiver) = std::sync::mpsc::sync_channel(config.queue_capacity);
        let (ready_sender, ready_receiver) = oneshot::channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(UiaState::Starting),
            active: Mutex::new(None),
            stopping: AtomicBool::new(false),
            finished: Notify::new(),
        });
        let id = NEXT_RUNTIME.fetch_add(1, Ordering::Relaxed);
        let worker_shared = shared.clone();
        let owner = crate::platform::Owner::uia()?;
        let thread = std::thread::Builder::new()
            .name(format!("argusflow-uia-{id}"))
            .spawn(move || {
                let _owner = owner;
                worker::run(receiver, ready_sender, worker_shared, config, id)
            })
            .map_err(|error| {
                Failure::new(FailureKind::Unavailable, "uia_start", "无法创建 UIA 线程")
                    .with_source(error)
            })?;
        let runtime = Self {
            inner: Arc::new(Inner {
                sender,
                shared,
                thread: Mutex::new(Some(thread)),
                id,
            }),
        };
        match tokio::time::timeout(operation.remaining(), ready_receiver).await {
            Ok(Ok(result)) => result?,
            Ok(Err(_)) => {
                return Err(Failure::new(
                    FailureKind::Unavailable,
                    "uia_start",
                    "UIA 初始化线程意外退出",
                ));
            }
            Err(_) => {
                return Err(Failure::new(
                    FailureKind::Timeout,
                    "uia_start",
                    "UIA 初始化超时",
                ));
            }
        }
        Ok(runtime)
    }
    /// 只读生命周期状态；Unresponsive 表示旧调用尚未回收。
    pub fn state(&self) -> UiaState {
        self.inner.shared.state()
    }
    /// 在窗口范围内查询多个元素。
    pub async fn find_all(
        &self,
        query: Query,
        options: OperationOptions,
    ) -> Result<Vec<ElementHandle>, Failure> {
        match self
            .call(worker::Command::Find(query, false), options)
            .await?
        {
            Response::Elements(elements) => Ok(elements),
            _ => Err(internal()),
        }
    }
    /// 在窗口范围内查询唯一元素，歧义直接报错。
    pub async fn find_unique(
        &self,
        query: Query,
        options: OperationOptions,
    ) -> Result<ElementHandle, Failure> {
        match self
            .call(worker::Command::Find(query, true), options)
            .await?
        {
            Response::Elements(mut elements) if elements.len() == 1 => Ok(elements.remove(0)),
            _ => Err(internal()),
        }
    }
    /// 读取元素属性，操作前重新校验归属与租约。
    pub async fn read(
        &self,
        element: &ElementHandle,
        options: OperationOptions,
    ) -> Result<ElementSnapshot, Failure> {
        self.validate_handle(element)?;
        match self
            .call(worker::Command::Read(element.clone()), options)
            .await?
        {
            Response::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(internal()),
        }
    }
    /// 执行指定 Pattern 动作；不自动重试。
    pub async fn perform(
        &self,
        element: &ElementHandle,
        action: UiaAction,
        options: OperationOptions,
    ) -> Result<(), Failure> {
        self.validate_handle(element)?;
        if matches!(&action, UiaAction::SetValue(text) if text.len() > 1_048_576) {
            return Err(Failure::new(
                FailureKind::ResourceLimit,
                "set_value",
                "输入文字超限",
            ));
        }
        match self
            .call(worker::Command::Act(element.clone(), action), options)
            .await?
        {
            Response::Done => Ok(()),
            _ => Err(internal()),
        }
    }
    fn validate_handle(&self, element: &ElementHandle) -> Result<(), Failure> {
        if element.runtime != self.inner.id || !element.lease.alive.load(Ordering::Acquire) {
            return Err(Failure::new(
                FailureKind::StaleHandle,
                "element_lease",
                "元素句柄不属于本实例或已被撤销",
            ));
        }
        Ok(())
    }
    async fn call(
        &self,
        command: worker::Command,
        options: OperationOptions,
    ) -> Result<Response, Failure> {
        if let worker::Command::Find(query, _) = &command {
            super::query::validate_predicate(&query.predicate, 0, &mut 0)?;
        }
        let operation = Operation::new(options);
        let mut guard = operation.cancel_on_drop();
        match self.state() {
            UiaState::Ready => {}
            UiaState::Unresponsive => {
                return Err(Failure::new(
                    FailureKind::Unresponsive,
                    "uia_dispatch",
                    "上一原生调用尚未退出",
                ));
            }
            _ => {
                return Err(Failure::new(
                    FailureKind::Unavailable,
                    "uia_dispatch",
                    "UIA 实例不可用",
                ));
            }
        }
        let (sender, receiver) = oneshot::channel();
        self.inner
            .sender
            .try_send(Request {
                command,
                operation: operation.clone(),
                response: sender,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    Failure::new(FailureKind::Busy, "uia_queue", "UIA 等待队列已满")
                }
                TrySendError::Disconnected(_) => {
                    Failure::new(FailureKind::Closed, "uia_queue", "UIA 线程已经退出")
                }
            })?;
        let result = match tokio::time::timeout(operation.remaining(), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Failure::new(
                FailureKind::Unavailable,
                "uia_response",
                "UIA 线程未返回响应",
            )),
            Err(_) => Err(Failure::new(
                FailureKind::Timeout,
                "uia_response",
                "UIA 操作总时限已到",
            )),
        };
        if result.is_ok() {
            operation.check("uia_response")?;
            guard.disarm();
        }
        result.map_err(|failure| {
            operation.contextualize(failure.with_resource(format!("uia:{}", self.inner.id)))
        })
    }
    /// 禁止新请求并等待 worker 真正退出；超时保留 Stopping 状态，不伪报已释放。
    pub async fn shutdown(&self, options: OperationOptions) -> Result<(), Failure> {
        self.inner.shared.stopping.store(true, Ordering::Release);
        if let Some(operation) = self
            .inner
            .shared
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            operation.cancel();
        }
        tokio::time::timeout(options.timeout(), async {
            loop {
                if self
                    .inner
                    .thread
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_ref()
                    .is_none_or(JoinHandle::is_finished)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| {
            Failure::new(
                FailureKind::Unresponsive,
                "uia_shutdown",
                "UIA 线程尚未退出",
            )
        })?;
        if let Some(thread) = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            thread.join().map_err(|_| {
                Failure::new(FailureKind::Native, "uia_shutdown", "UIA 线程发生 panic")
            })?;
        }
        Ok(())
    }
}

fn internal() -> Failure {
    Failure::new(FailureKind::Protocol, "uia_response", "UIA 响应类型错误")
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/uia/runtime.rs"]
mod tests;
