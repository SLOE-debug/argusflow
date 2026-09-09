//! 有界原生推理线程与可取消的异步接口。
use crate::OcrError as Failure;
use crate::{
    ImageInput, OcrConfig, OcrResult,
    model::{ActiveRun, Models},
    ocr,
};
use argusflow_core::{FailureKind, Operation, OperationOptions};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::sync::oneshot;

/// OCR 引擎的只读状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrState {
    /// 模型正在加载。
    Loading,
    /// 可接受请求。
    Ready,
    /// 已取消的原生调用仍未退出。
    Unresponsive,
    /// 正在关闭。
    Stopping,
    /// 模型和线程已释放。
    Stopped,
    /// 初始化或线程失败。
    Failed,
}
struct Shared {
    state: Mutex<OcrState>,
    active: Mutex<Option<Operation>>,
    run: ActiveRun,
    stopping: AtomicBool,
}
impl Shared {
    fn state(&self) -> OcrState {
        let state = *self.state.lock().unwrap_or_else(|p| p.into_inner());
        if self.stopping.load(Ordering::Acquire)
            && !matches!(state, OcrState::Stopped | OcrState::Failed)
        {
            return OcrState::Stopping;
        }
        if state == OcrState::Ready
            && self
                .active
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_some_and(|operation| {
                    operation.is_cancelled() || operation.remaining().is_zero()
                })
        {
            OcrState::Unresponsive
        } else {
            state
        }
    }
    fn cancel(&self, operation: &Operation) {
        operation.cancel();
        // 同步持有 active 锁直到 terminate 完成，禁止取消排队请求时误伤另一个请求。
        let active = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if active
            .as_ref()
            .is_some_and(|current| current.id() == operation.id())
            && let Some(run) = self.run.lock().unwrap_or_else(|p| p.into_inner()).as_ref()
        {
            let _ = run.terminate();
        }
    }
    fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let active = self
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(operation) = active {
            self.cancel(&operation);
        }
    }
}
struct Request {
    input: ImageInput,
    operation: Operation,
    response: oneshot::Sender<Result<OcrResult, Failure>>,
}
struct Inner {
    sender: SyncSender<Request>,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
    config: OcrConfig,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.shared.stop();
        if let Some(thread) = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
    }
}
struct CancelGuard {
    shared: Arc<Shared>,
    operation: Operation,
    armed: bool,
}
impl Drop for CancelGuard {
    fn drop(&mut self) {
        if self.armed {
            self.shared.cancel(&self.operation);
        }
    }
}

/// 可复用 OCR 引擎；一次加载只建立一个原生推理线程。
#[derive(Clone)]
pub struct OcrEngine {
    inner: Arc<Inner>,
}
impl OcrEngine {
    /// 显式加载所选模型；默认模型加载时限为 120 秒。
    pub async fn load(config: OcrConfig) -> Result<Self, Failure> {
        Self::load_with_options(config, OperationOptions::new(Duration::from_secs(120))?).await
    }
    /// 按指定总时限加载模型；取消后不创建替代线程。
    pub async fn load_with_options(
        config: OcrConfig,
        options: OperationOptions,
    ) -> Result<Self, Failure> {
        config.validate()?;
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        let (sender, receiver) = mpsc::sync_channel(config.queue_capacity);
        let (ready_sender, ready_receiver) = oneshot::channel();
        let shared = Arc::new(Shared {
            state: Mutex::new(OcrState::Loading),
            active: Mutex::new(None),
            run: Arc::default(),
            stopping: AtomicBool::new(false),
        });
        let worker_shared = shared.clone();
        let worker_config = config.clone();
        let worker_operation = operation.clone();
        let owner = super::ownership::Owner::acquire(&config)?;
        let thread = std::thread::Builder::new()
            .name(format!("argusflow-ocr-{}", config.tier.directory()))
            .spawn(move || {
                let _owner = owner;
                worker(
                    receiver,
                    ready_sender,
                    worker_shared,
                    worker_config,
                    worker_operation,
                )
            })
            .map_err(|error| {
                Failure::new(FailureKind::Unavailable, "ocr_start", "无法创建推理线程")
                    .with_source(error)
            })?;
        let engine = Self {
            inner: Arc::new(Inner {
                sender,
                shared,
                thread: Mutex::new(Some(thread)),
                config,
            }),
        };
        match tokio::time::timeout(operation.remaining(), ready_receiver).await {
            Ok(Ok(result)) => result?,
            Ok(Err(_)) => {
                return Err(Failure::new(
                    FailureKind::Unavailable,
                    "ocr_load",
                    "模型加载线程未返回结果",
                ));
            }
            Err(_) => {
                return Err(Failure::new(
                    FailureKind::Timeout,
                    "ocr_load",
                    "模型加载超时，线程正在协作退出",
                ));
            }
        }
        Ok(engine)
    }
    /// 当前只读状态；不因查询状态而重建模型。
    pub fn state(&self) -> OcrState {
        self.inner.shared.state()
    }
    /// 识别一张图片，默认总时限 60 秒。
    pub async fn recognize(&self, input: ImageInput) -> Result<OcrResult, Failure> {
        self.recognize_with_options(input, OperationOptions::new(Duration::from_secs(60))?)
            .await
    }
    /// 使用包含排队的指定总时限识别；丢弃 Future 将取消后续阶段及当前 ORT Run。
    pub async fn recognize_with_options(
        &self,
        input: ImageInput,
        options: OperationOptions,
    ) -> Result<OcrResult, Failure> {
        input.validate(&self.inner.config)?;
        let operation = Operation::new(options);
        let mut guard = CancelGuard {
            shared: self.inner.shared.clone(),
            operation: operation.clone(),
            armed: true,
        };
        match self.state() {
            OcrState::Ready => {}
            OcrState::Unresponsive => {
                return Err(Failure::new(
                    FailureKind::Unresponsive,
                    "ocr_dispatch",
                    "上一原生推理尚未退出",
                ));
            }
            _ => {
                return Err(Failure::new(
                    FailureKind::Unavailable,
                    "ocr_dispatch",
                    "OCR 实例不可用",
                ));
            }
        }
        let (sender, receiver) = oneshot::channel();
        self.inner
            .sender
            .try_send(Request {
                input,
                operation: operation.clone(),
                response: sender,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    Failure::new(FailureKind::Busy, "ocr_queue", "OCR 等待队列已满")
                }
                TrySendError::Disconnected(_) => {
                    Failure::new(FailureKind::Closed, "ocr_queue", "OCR 线程已退出")
                }
            })?;
        let result = match tokio::time::timeout(operation.remaining(), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Failure::new(
                FailureKind::Unavailable,
                "ocr_response",
                "OCR 线程未返回结果",
            )),
            Err(_) => Err(Failure::new(
                FailureKind::Timeout,
                "ocr_response",
                "OCR 总截止时间已到",
            )),
        };
        if result.is_ok() {
            operation.check("ocr_response")?;
            guard.armed = false;
        }
        result.map_err(|failure| {
            operation.contextualize(failure.with_resource(format!(
                "ocr:{:?}:{:?}",
                self.inner.config.tier, self.inner.config.device
            )))
        })
    }
    /// 停止排队、取消当前推理，并有界等待模型和线程释放。
    pub async fn shutdown(&self, options: OperationOptions) -> Result<(), Failure> {
        self.inner.shared.stop();
        tokio::time::timeout(options.timeout(), async {
            while self
                .inner
                .thread
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .is_some_and(|thread| !thread.is_finished())
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| {
            Failure::new(
                FailureKind::Unresponsive,
                "ocr_shutdown",
                "原生推理尚未退出，资源尚未释放",
            )
        })?;
        if let Some(thread) = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            thread
                .join()
                .map_err(|_| Failure::new(FailureKind::Native, "ocr_shutdown", "OCR 线程 panic"))?;
        }
        Ok(())
    }
}

fn worker(
    receiver: mpsc::Receiver<Request>,
    ready: oneshot::Sender<Result<(), Failure>>,
    shared: Arc<Shared>,
    config: OcrConfig,
    operation: Operation,
) {
    let loading = operation.clone();
    worker_with(
        receiver,
        ready,
        shared,
        operation,
        || Models::load(&config, &loading),
        |models, input, operation, active| {
            ocr::recognize(models, input, &config, operation, active)
        },
    );
}

fn worker_with<B>(
    receiver: mpsc::Receiver<Request>,
    ready: oneshot::Sender<Result<(), Failure>>,
    shared: Arc<Shared>,
    operation: Operation,
    create: impl FnOnce() -> Result<B, Failure>,
    mut recognize: impl FnMut(&mut B, ImageInput, &Operation, &ActiveRun) -> Result<OcrResult, Failure>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut models = match create() {
            Ok(models) => models,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        if operation.check("ocr_load_complete").is_err() || shared.stopping.load(Ordering::Acquire)
        {
            return;
        }
        *shared.state.lock().unwrap_or_else(|p| p.into_inner()) = OcrState::Ready;
        if ready.send(Ok(())).is_err() {
            return;
        }
        loop {
            if shared.stopping.load(Ordering::Acquire) {
                break;
            }
            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok(request) => {
                    if shared.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(request.operation.clone());
                    let started = std::time::Instant::now();
                    let result = request
                        .operation
                        .check("ocr_execute")
                        .map_err(Failure::from)
                        .and_then(|_| {
                            recognize(&mut models, request.input, &request.operation, &shared.run)
                        })
                        .map_err(|failure| request.operation.contextualize(failure));
                    tracing::debug!(
                        request_id = request.operation.id(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        success = result.is_ok(),
                        "ocr request completed"
                    );
                    *shared.run.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    let _ = request.response.send(
                        request
                            .operation
                            .check("ocr_response")
                            .map_err(Failure::from)
                            .and(result),
                    );
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(models);
    }));
    *shared.run.lock().unwrap_or_else(|p| p.into_inner()) = None;
    *shared.active.lock().unwrap_or_else(|p| p.into_inner()) = None;
    *shared.state.lock().unwrap_or_else(|p| p.into_inner()) = if result.is_ok() {
        OcrState::Stopped
    } else {
        OcrState::Failed
    };
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-vision/unit/engine/service.rs"]
mod tests;
