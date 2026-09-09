//! 有界真实输入服务及原生线程生命周期。
use crate::WindowsError as Failure;

use super::inject;
use crate::WindowIdentity;
use argusflow_core::{
    ClickCount, FailureKind, Key, MouseButton, Operation, OperationOptions, ScreenPoint, ScrollAxis,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    time::Duration,
};
use tokio::sync::oneshot;

/// 显式的真实输入操作，坐标均为屏幕物理像素。
#[derive(Debug, Clone)]
pub enum InputAction {
    /// 移动鼠标。
    Move(ScreenPoint),
    /// 点击目标位置。
    Click {
        /// 点击点。
        point: ScreenPoint,
        /// 鼠标按键。
        button: MouseButton,
        /// 单击或双击。
        count: ClickCount,
    },
    /// 在指定位置发送滚轮事件；每一步为 WHEEL_DELTA=120。
    Wheel {
        /// 鼠标位置。
        point: ScreenPoint,
        /// 滚动轴。
        axis: ScrollAxis,
        /// 非零有符号步数，垂直正数向上、水平正数向右。
        steps: i32,
    },
    /// Unicode 文字输入，不通过剪贴板。
    Text(String),
    /// 同时按下后逆序释放的组合键，最多八个且不得重复。
    Chord(Vec<Key>),
}

struct Request {
    window: WindowIdentity,
    action: InputAction,
    operation: Operation,
    response: oneshot::Sender<Result<(), Failure>>,
}
struct State {
    stopping: AtomicBool,
    stopped: AtomicBool,
    active: Mutex<Option<Operation>>,
}
struct Inner {
    sender: SyncSender<Request>,
    state: Arc<State>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::Release);
        if let Some(operation) = self
            .state
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            operation.cancel();
        }
    }
}

/// 复用一个有界输入线程；Clone 不建立额外输入流。
#[derive(Clone)]
pub struct InputService {
    inner: Arc<Inner>,
}

/// 真实输入线程的只读状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputState {
    /// 可以提交输入。
    Ready,
    /// 取消的原生调用尚未退出。
    Unresponsive,
    /// 禁止新输入，正在清理。
    Stopping,
    /// 输入线程已经退出。
    Stopped,
}

impl InputService {
    /// 查询状态，不改变前台窗口或输入状态。
    pub fn state(&self) -> InputState {
        let state = &self.inner.state;
        if state.stopped.load(Ordering::Acquire) {
            InputState::Stopped
        } else if state.stopping.load(Ordering::Acquire) {
            InputState::Stopping
        } else if state
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|op| op.check("input_state").is_err())
        {
            InputState::Unresponsive
        } else {
            InputState::Ready
        }
    }
    /// 建立容量为 64 的输入队列，不改变当前焦点。
    pub fn new() -> Result<Self, Failure> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(64);
        let state = Arc::new(State {
            stopping: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            active: Mutex::new(None),
        });
        let worker_state = state.clone();
        let owner = crate::platform::Owner::input()?;
        let thread = std::thread::Builder::new()
            .name("argusflow-input".into())
            .spawn(move || {
                let _owner = owner;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    loop {
                        if worker_state.stopping.load(Ordering::Acquire) {
                            break;
                        }
                        match receiver.recv_timeout(Duration::from_millis(25)) {
                            Ok(request) => {
                                if worker_state.stopping.load(Ordering::Acquire) {
                                    break;
                                }
                                *worker_state
                                    .active
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner()) =
                                    Some(request.operation.clone());
                                let result = inject::perform(
                                    request.window,
                                    request.action,
                                    &request.operation,
                                )
                                .map_err(|error| request.operation.contextualize(error));
                                let _ = request.response.send(result);
                                *worker_state
                                    .active
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner()) = None;
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                }));
                if result.is_err() {
                    worker_state.stopping.store(true, Ordering::Release);
                }
                *worker_state
                    .active
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = None;
                worker_state.stopped.store(true, Ordering::Release);
            })
            .map_err(|error| {
                Failure::new(FailureKind::Unavailable, "input_start", "无法创建输入线程")
                    .with_source(error)
            })?;
        Ok(Self {
            inner: Arc::new(Inner {
                sender,
                state,
                thread: Mutex::new(Some(thread)),
            }),
        })
    }
    /// 提交一次真实输入；目标必须已经在前台，不自动激活窗口。
    pub async fn perform(
        &self,
        window: WindowIdentity,
        action: InputAction,
        options: OperationOptions,
    ) -> Result<(), Failure> {
        if matches!(&action,InputAction::Text(text) if text.is_empty() || text.len()>16_384)
            || matches!(&action,InputAction::Chord(keys) if keys.is_empty() || keys.len()>8)
        {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "input_validate",
                "文字必须为 1-16384 字节，组合键必须为 1-8 个",
            ));
        }
        let operation = Operation::new(options);
        let mut guard = operation.cancel_on_drop();
        if self.inner.state.stopping.load(Ordering::Acquire) {
            return Err(Failure::new(
                FailureKind::Closed,
                "input",
                "输入服务正在关闭",
            ));
        }
        if self
            .inner
            .state
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .is_some_and(|op| op.check("input").is_err())
        {
            return Err(Failure::new(
                FailureKind::Unresponsive,
                "input",
                "上一输入调用尚未退出",
            ));
        }
        let (sender, receiver) = oneshot::channel();
        self.inner
            .sender
            .try_send(Request {
                window,
                action,
                operation: operation.clone(),
                response: sender,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => {
                    Failure::new(FailureKind::Busy, "input_queue", "输入队列已满")
                }
                TrySendError::Disconnected(_) => {
                    Failure::new(FailureKind::Closed, "input_queue", "输入线程已退出")
                }
            })?;
        let result = match tokio::time::timeout(operation.remaining(), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Failure::new(
                FailureKind::Closed,
                "input_response",
                "输入线程没有返回结果",
            )),
            Err(_) => Err(Failure::new(
                FailureKind::Timeout,
                "input_response",
                "输入操作总时限已到",
            )),
        };
        if result.is_ok() {
            operation.check("input_complete").map_err(Failure::from)?;
            guard.disarm();
        }
        result.map_err(|failure| operation.contextualize(failure))
    }
    /// 取消未完成操作并等待输入线程退出。
    pub async fn shutdown(&self, options: OperationOptions) -> Result<(), Failure> {
        self.inner.state.stopping.store(true, Ordering::Release);
        if let Some(operation) = self
            .inner
            .state
            .active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            operation.cancel();
        }
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
                "input_shutdown",
                "原生输入线程尚未退出",
            )
        })?;
        if let Some(thread) = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = thread.join();
        }
        Ok(())
    }
}
