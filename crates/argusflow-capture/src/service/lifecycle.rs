//! 显式服务生命周期及唯一元数据消费任务。
use super::State;
use crate::CaptureConfig;
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, Semaphore};

pub(crate) struct Inner {
    pub backend: Arc<dyn DesktopBackend>,
    pub config: CaptureConfig,
    pub state: Mutex<State>,
    pub notify: Notify,
    pub observations: Arc<Semaphore>,
    pub reads: Arc<Semaphore>,
    pub stop: AtomicBool,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.backend.release_consumer();
    }
}
/// 可克隆共享采样服务；启动时要求存在 Tokio runtime。
#[derive(Clone)]
pub struct CaptureService {
    pub(crate) inner: Arc<Inner>,
}
impl CaptureService {
    /// 注入已经启动的原生后端，建立唯一事件消费任务。
    pub fn start(backend: Arc<dyn DesktopBackend>, config: CaptureConfig) -> CaptureResult<Self> {
        config.validate()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
            CaptureError::new(
                FailureKind::Unavailable,
                "capture_start",
                "没有 Tokio runtime",
            )
        })?;
        backend.claim_consumer()?;
        let session = backend.clock().session;
        let inner = Arc::new(Inner {
            backend,
            observations: Arc::new(Semaphore::new(config.observations)),
            reads: Arc::new(Semaphore::new(config.reads)),
            config,
            state: Mutex::new(State::new(session)),
            notify: Notify::new(),
            stop: AtomicBool::new(false),
            task: Mutex::new(None),
        });
        let weak = Arc::downgrade(&inner);
        let task = runtime.spawn(async move {
            loop {
                let Some(inner) = weak.upgrade() else {
                    break;
                };
                if inner.stop.load(Ordering::Acquire) {
                    break;
                }
                let result = inner.backend.poll();
                {
                    let mut state = inner.state.lock().unwrap_or_else(|p| p.into_inner());
                    match result {
                        Ok(events) => {
                            for event in events {
                                state.apply(event, inner.backend.now(), &inner.config);
                            }
                        }
                        Err(error) => {
                            state.failure = Some(error);
                            inner.stop.store(true, Ordering::Release);
                        }
                    }
                    state.expire(inner.backend.now(), &inner.config);
                }
                inner.notify.notify_waiters();
                drop(inner);
                tokio::time::sleep(std::time::Duration::from_millis(4)).await;
            }
        });
        *inner.task.lock().unwrap_or_else(|p| p.into_inner()) = Some(task);
        Ok(Self { inner })
    }
    /// 当前时钟域。
    pub fn clock(&self) -> ClockDomain {
        self.inner.backend.clock()
    }
    /// 当前单调时间。
    pub fn now(&self) -> ClockTime {
        self.inner.backend.now()
    }
    /// 查询所有已发现的输出及状态。
    pub fn sources(&self) -> Vec<SourceInfo> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sources
            .values()
            .map(|entry| entry.info.clone())
            .collect()
    }
    /// 原生工作量和实际资源统计。
    pub fn stats(&self) -> CaptureStats {
        self.inner.backend.stats()
    }
    /// 请求原生后端新的一轮有限恢复。
    pub fn restart(&self) -> CaptureResult<()> {
        self.inner.backend.restart()
    }
    /// 停止新请求并有界回收共享服务与原生后台。
    pub async fn shutdown(&self, options: OperationOptions) -> CaptureResult<()> {
        self.inner.stop.store(true, Ordering::Release);
        self.inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stopped = true;
        self.inner.notify.notify_waiters();
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        self.inner.backend.shutdown(operation.clone()).await?;
        let task = self
            .inner
            .task
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(task) = task {
            tokio::time::timeout(operation.remaining(), task)
                .await
                .map_err(|_| {
                    CaptureError::new(
                        FailureKind::Unresponsive,
                        "capture_shutdown",
                        "服务任务未退出",
                    )
                })?
                .map_err(|_| {
                    CaptureError::new(FailureKind::Native, "capture_shutdown", "服务任务失败")
                })?;
        }
        Ok(())
    }
    pub(crate) async fn wait_update(&self, operation: &Operation) -> CaptureResult<()> {
        operation.check("capture_wait")?;
        tokio::select! {_ = self.inner.notify.notified()=>{},_ = tokio::time::sleep(operation.remaining().min(std::time::Duration::from_millis(8)))=>{}}
        operation.check("capture_wait")?;
        Ok(())
    }
}
