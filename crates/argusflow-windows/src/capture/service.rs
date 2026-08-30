//! 非阻塞初始化的应用级 Windows Graphics Capture 服务。

use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{
    CaptureHealth, CaptureLifecycle, CapturePolicy, FrameSubscription, VisionError,
    WindowFrameSource,
};
use async_trait::async_trait;

use super::{host::WindowsCaptureHost, wgc::WindowsGraphicsCapture};

/// 可在 Tauri 首屏完成后异步初始化的 WGC 服务门面。
#[derive(Debug)]
pub struct WindowsCaptureService {
    /// 当前捕获主机及其生命周期状态。
    state: RwLock<CaptureServiceState>,
    /// 尚未回收的初始化线程，确保退出时不会遗留图形资源。
    initialization: Mutex<Option<JoinHandle<()>>>,
    /// 应用退出后拒绝发布迟到的初始化结果。
    shutdown_requested: AtomicBool,
}

/// 捕获服务内部状态；Ready 同时拥有主机和供调用方克隆的帧源。
#[derive(Debug)]
enum CaptureServiceState {
    Pending,
    Initializing,
    Ready {
        host: WindowsCaptureHost,
        source: WindowsGraphicsCapture,
    },
    Failed(String),
    Stopped,
}

impl WindowsCaptureService {
    /// 创建尚未初始化图形资源的轻量服务。
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(CaptureServiceState::Pending),
            initialization: Mutex::new(None),
            shutdown_requested: AtomicBool::new(false),
        })
    }

    /// 在专用线程启动 WinRT、D3D11 与 WGC 主机，不阻塞调用线程。
    pub fn start(self: &Arc<Self>) -> Result<(), VisionError> {
        self.reap_finished_initialization()?;
        {
            let mut state = self
                .state
                .write()
                .map_err(|_| service_error("capture service state lock was poisoned"))?;
            if matches!(
                *state,
                CaptureServiceState::Initializing | CaptureServiceState::Ready { .. }
            ) {
                return Ok(());
            }
            self.shutdown_requested.store(false, Ordering::Release);
            *state = CaptureServiceState::Initializing;
        }

        let service = self.clone();
        let thread = thread::Builder::new()
            .name("argusflow-wgc-initializer".to_owned())
            .spawn(move || service.initialize())
            .map_err(|error| {
                let message = format!("failed to start capture initializer: {error}");
                service_error(message)
            });
        match thread {
            Ok(thread) => {
                let mut initialization = self.initialization.lock().map_err(|_| {
                    service_error("capture initialization handle lock was poisoned")
                })?;
                *initialization = Some(thread);
                Ok(())
            }
            Err(error) => {
                self.set_failed(error.to_string());
                Err(error)
            }
        }
    }

    /// 清除失败状态并重新初始化捕获主机。
    pub fn retry(self: &Arc<Self>) -> Result<(), VisionError> {
        self.start()
    }

    /// 停止初始化或已就绪的捕获主机，并等待相关线程退出。
    pub fn shutdown(&self) -> Result<(), VisionError> {
        self.shutdown_requested.store(true, Ordering::Release);
        let initialization = self
            .initialization
            .lock()
            .map_err(|_| service_error("capture initialization handle lock was poisoned"))?
            .take();
        if let Some(initialization) = initialization {
            initialization
                .join()
                .map_err(|_| service_error("capture initializer panicked during shutdown"))?;
        }

        let previous = {
            let mut state = self
                .state
                .write()
                .map_err(|_| service_error("capture service state lock was poisoned"))?;
            std::mem::replace(&mut *state, CaptureServiceState::Stopped)
        };
        if let CaptureServiceState::Ready { host, .. } = previous {
            host.shutdown()?;
        }
        Ok(())
    }

    /// 完成阻塞式 WGC 初始化，并仅在应用仍运行时发布主机。
    fn initialize(&self) {
        match WindowsCaptureHost::start() {
            Ok(host) => {
                if self.shutdown_requested.load(Ordering::Acquire) {
                    let _ = host.shutdown();
                    return;
                }
                let source = host.frame_source();
                let Ok(mut state) = self.state.write() else {
                    let _ = host.shutdown();
                    return;
                };
                *state = CaptureServiceState::Ready { host, source };
            }
            Err(error) => self.set_failed(error.to_string()),
        }
    }

    /// 回收已经结束的初始化线程，以允许失败后再次启动。
    fn reap_finished_initialization(&self) -> Result<(), VisionError> {
        let finished = {
            let mut initialization = self
                .initialization
                .lock()
                .map_err(|_| service_error("capture initialization handle lock was poisoned"))?;
            if initialization.as_ref().is_some_and(JoinHandle::is_finished) {
                initialization.take()
            } else {
                None
            }
        };
        if let Some(finished) = finished {
            finished
                .join()
                .map_err(|_| service_error("capture initializer panicked"))?;
        }
        Ok(())
    }

    /// 在初始化失败时保留可展示的稳定摘要。
    fn set_failed(&self, message: String) {
        if let Ok(mut state) = self.state.write() {
            *state = CaptureServiceState::Failed(message);
        }
    }

    /// 取得已就绪帧源的克隆，避免跨 await 持有状态锁。
    fn ready_source(&self) -> Result<WindowsGraphicsCapture, VisionError> {
        let state = self
            .state
            .read()
            .map_err(|_| service_error("capture service state lock was poisoned"))?;
        match &*state {
            CaptureServiceState::Ready { source, .. } => Ok(source.clone()),
            CaptureServiceState::Failed(message) => Err(service_error(message.clone())),
            CaptureServiceState::Pending => Err(service_error("capture initialization is pending")),
            CaptureServiceState::Initializing => {
                Err(service_error("capture initialization is still running"))
            }
            CaptureServiceState::Stopped => Err(service_error("capture service is stopped")),
        }
    }
}

#[async_trait]
impl WindowFrameSource for WindowsCaptureService {
    fn health(&self) -> CaptureHealth {
        let Ok(state) = self.state.read() else {
            return CaptureHealth::failed("capture service state lock was poisoned");
        };
        match &*state {
            CaptureServiceState::Pending => CaptureHealth::new(CaptureLifecycle::Pending),
            CaptureServiceState::Initializing => CaptureHealth::new(CaptureLifecycle::Initializing),
            CaptureServiceState::Ready { .. } => CaptureHealth::new(CaptureLifecycle::Ready),
            CaptureServiceState::Failed(message) => CaptureHealth::failed(message.clone()),
            CaptureServiceState::Stopped => CaptureHealth::failed("capture service is stopped"),
        }
    }

    async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        self.ready_source()?.open(window, policy).await
    }
}

/// 构造视觉层统一的捕获不可用错误。
fn service_error(message: impl Into<String>) -> VisionError {
    VisionError::CaptureUnavailable {
        message: message.into(),
    }
}
