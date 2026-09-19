//! 单工作线程隔离延迟渲染；超时后不启动替代线程，直到原调用退出。
use crate::WindowsError;
use argusflow_core::{FailureKind, Operation, OperationOptions};
use argusflow_input_contracts::ClipboardObservation;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use tokio::sync::{Notify, oneshot};

struct Request {
    previous: Option<u32>,
    operation: Operation,
    reply: oneshot::Sender<Result<ClipboardObservation, WindowsError>>,
}
struct State {
    busy: AtomicBool,
    stopping: AtomicBool,
    stopped: AtomicBool,
    finished: Notify,
}
struct Inner {
    sender: mpsc::SyncSender<Request>,
    state: Arc<State>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.state.stopping.store(true, Ordering::Release);
    }
}
/// 应用范围内复用的只读服务；最多一个在途请求，不缓存文本。
#[derive(Clone)]
pub struct ClipboardReader {
    inner: Arc<Inner>,
}
impl ClipboardReader {
    /// 创建一个工作线程，未就绪的请求不会扩散到 UIA 线程。
    pub fn start() -> Result<Self, WindowsError> {
        Self::start_with(super::read::read)
    }
    // 来源运行在唯一工作线程，便于独立验证阻塞和取消的生命周期边界。
    fn start_with(
        mut read: impl FnMut(Option<u32>, &Operation) -> Result<ClipboardObservation, WindowsError>
        + Send
        + 'static,
    ) -> Result<Self, WindowsError> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        let state = Arc::new(State {
            busy: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            finished: Notify::new(),
        });
        let worker = state.clone();
        std::thread::Builder::new()
            .name("argusflow-clipboard".into())
            .spawn(move || {
                while !worker.stopping.load(Ordering::Acquire) {
                    match receiver.recv_timeout(std::time::Duration::from_millis(25)) {
                        Ok(request) => {
                            let result = read(request.previous, &request.operation);
                            worker.busy.store(false, Ordering::Release);
                            let _ = request.reply.send(result);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                worker.stopped.store(true, Ordering::Release);
                worker.finished.notify_waiters();
            })
            .map_err(|e| {
                WindowsError::new(
                    FailureKind::Unavailable,
                    "clipboard_start",
                    "无法创建剪贴板线程",
                )
                .with_source(e)
            })?;
        Ok(Self {
            inner: Arc::new(Inner { sender, state }),
        })
    }
    /// 读取一次稳定序号快照。previous 只传上一次成功结果；失败不能推进基线。
    pub async fn observe(
        &self,
        previous: Option<u32>,
        options: OperationOptions,
    ) -> Result<ClipboardObservation, WindowsError> {
        let operation = Operation::new(options);
        let _cancel = operation.cancel_on_drop();
        operation.check("clipboard_dispatch")?;
        let state = &self.inner.state;
        if state.stopping.load(Ordering::Acquire) {
            return Err(WindowsError::new(
                FailureKind::Closed,
                "clipboard_dispatch",
                "剪贴板服务已关闭",
            ));
        }
        if state.busy.swap(true, Ordering::AcqRel) {
            return Err(WindowsError::new(
                FailureKind::Busy,
                "clipboard_dispatch",
                "上次剪贴板读取尚未退出",
            ));
        }
        let (reply, mut receiver) = oneshot::channel();
        if self
            .inner
            .sender
            .try_send(Request {
                previous,
                operation: operation.clone(),
                reply,
            })
            .is_err()
        {
            state.busy.store(false, Ordering::Release);
            return Err(WindowsError::new(
                FailureKind::Closed,
                "clipboard_dispatch",
                "剪贴板工作线程不可用",
            ));
        }
        loop {
            tokio::select! {
                result = &mut receiver => { operation.check("clipboard_response")?; return result.map_err(|_| WindowsError::new(FailureKind::Closed,"clipboard_response","剪贴板工作线程已退出"))?; },
                _ = tokio::time::sleep(operation.remaining().min(std::time::Duration::from_millis(8))) => operation.check("clipboard_response")?,
            }
        }
    }
    /// 禁止新请求并等待真实退出；超时不宣称延迟渲染已被终止。
    pub async fn shutdown(&self, options: OperationOptions) -> Result<(), WindowsError> {
        self.inner.state.stopping.store(true, Ordering::Release);
        let operation = Operation::new(options);
        loop {
            let notified = self.inner.state.finished.notified();
            if self.inner.state.stopped.load(Ordering::Acquire) {
                return Ok(());
            }
            operation.check("clipboard_shutdown")?;
            tokio::select! { _ = notified => {}, _ = tokio::time::sleep(operation.remaining().min(std::time::Duration::from_millis(8))) => {} }
        }
    }
}
#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/clipboard/service.rs"]
mod tests;
