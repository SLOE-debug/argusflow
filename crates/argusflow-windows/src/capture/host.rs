//! 应用级 WGC 主机线程、WinRT apartment 与捕获会话所有权。

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Sender, SyncSender, channel, sync_channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use argusflow_vision::{CapturePolicy, CapturedFrame, FrameId, TopologyGeneration, VisionError};
use tokio::sync::{Notify, oneshot};

use super::{host_thread::run_capture_thread, wgc::WindowsGraphicsCapture};

/// 应用级 Windows Graphics Capture 主机。
///
/// 主机在创建时启动唯一 MTA 线程并初始化共享 D3D11 设备。所有 WGC session、
/// frame pool、拓扑和 GPU readback 都由该线程拥有，退出时按确定顺序统一销毁。
#[derive(Debug)]
pub struct WindowsCaptureHost {
    /// 捕获源与订阅共享的线程命令入口。
    client: Arc<CaptureHostClient>,
    /// 唯一主机线程；Option 保证 shutdown/join 只执行一次。
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl WindowsCaptureHost {
    /// 启动应用级 MTA 捕获线程，并在返回前完成 WinRT 与 D3D11 初始化。
    pub fn start() -> Result<Self, VisionError> {
        let (command_sender, command_receiver) = channel();
        // 启动握手确保 AppState 可见前，捕获线程及共享设备已经完全就绪。
        let (ready_sender, ready_receiver) = sync_channel(1);
        let thread = thread::Builder::new()
            .name("argusflow-wgc-capture".to_owned())
            .spawn(move || run_capture_thread(command_receiver, ready_sender))
            .map_err(|error| {
                capture_host_error(format!("failed to start capture thread: {error}"))
            })?;
        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                client: Arc::new(CaptureHostClient {
                    commands: command_sender,
                    accepting_commands: AtomicBool::new(true),
                }),
                thread: Mutex::new(Some(thread)),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(capture_host_error(
                    "capture thread stopped before startup completed",
                ))
            }
        }
    }

    /// 返回绑定当前应用级主机的 WGC 帧源。
    pub fn frame_source(&self) -> WindowsGraphicsCapture {
        WindowsGraphicsCapture::from_client(self.client.clone())
    }

    /// 停止接收新任务，在主机线程上销毁全部 session/pool/device，并等待线程退出。
    pub fn shutdown(&self) -> Result<(), VisionError> {
        let mut thread_slot = self
            .thread
            .lock()
            .map_err(|_| capture_host_error("capture host lifecycle mutex was poisoned"))?;
        let Some(thread) = thread_slot.take() else {
            return Ok(());
        };
        self.client
            .accepting_commands
            .store(false, Ordering::Release);
        // 同步确认由主机线程发送，保证 join 前所有 WGC 对象已经在其创建线程上销毁。
        let (reply, response) = sync_channel(1);
        let send_result = self
            .client
            .commands
            .send(CaptureCommand::Shutdown { reply });
        let shutdown_result = match send_result {
            Ok(()) => response.recv().map_err(|_| {
                capture_host_error("capture thread stopped before shutdown completed")
            }),
            Err(_) => Err(capture_host_error(
                "capture thread was unavailable during shutdown",
            )),
        };
        drop(thread_slot);
        let join_result = thread
            .join()
            .map_err(|_| capture_host_error("capture thread panicked during shutdown"));
        shutdown_result.and(join_result)
    }
}

impl Drop for WindowsCaptureHost {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// 可跨异步任务克隆的主机命令入口；不暴露线程和资源所有权。
#[derive(Debug)]
pub(super) struct CaptureHostClient {
    /// 发往唯一捕获线程的串行命令队列。
    commands: Sender<CaptureCommand>,
    /// shutdown 开始后立即拒绝新 open/poll 请求。
    accepting_commands: AtomicBool,
}

impl CaptureHostClient {
    /// 在主机线程上建立窗口订阅，并返回轻量订阅句柄。
    pub(super) async fn open(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<OpenedCapture, VisionError> {
        let (reply, response) = oneshot::channel();
        self.send(CaptureCommand::Open {
            window,
            policy,
            reply,
        })?;
        response.await.map_err(|_| host_stopped())?
    }

    /// 在主机线程上轮询一张单 HWND 捕获帧。
    pub(super) async fn poll(
        &self,
        subscription_id: CaptureSubscriptionId,
        frame_id: FrameId,
        deadline: Instant,
        timeout: Duration,
    ) -> Result<Option<Arc<CapturedFrame>>, VisionError> {
        let (reply, response) = oneshot::channel();
        self.send(CaptureCommand::Poll {
            subscription_id,
            frame_id,
            deadline,
            timeout,
            reply,
        })?;
        response.await.map_err(|_| host_stopped())?
    }

    /// 在主机线程上刷新拓扑并返回当前代数。
    pub(super) async fn current_topology_generation(
        &self,
        subscription_id: CaptureSubscriptionId,
    ) -> Result<TopologyGeneration, VisionError> {
        let (reply, response) = oneshot::channel();
        self.send(CaptureCommand::CurrentTopology {
            subscription_id,
            reply,
        })?;
        response.await.map_err(|_| host_stopped())?
    }

    /// 订阅释放时请求主机线程销毁其 WGC 资源；应用退出后无需重复发送。
    pub(super) fn close(&self, subscription_id: CaptureSubscriptionId) {
        if self.accepting_commands.load(Ordering::Acquire) {
            let _ = self
                .commands
                .send(CaptureCommand::Close { subscription_id });
        }
    }

    /// 仅在主机仍接受业务命令时入队，避免退出阶段创建新资源。
    fn send(&self, command: CaptureCommand) -> Result<(), VisionError> {
        if !self.accepting_commands.load(Ordering::Acquire) {
            return Err(host_stopped());
        }
        self.commands.send(command).map_err(|_| host_stopped())
    }
}

/// 主机内部使用的强类型订阅标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CaptureSubscriptionId(pub(super) u64);

/// 打开订阅后返回给异步代理的最小只读契约。
#[derive(Debug, Clone)]
pub(super) struct OpenedCapture {
    /// 主机表中的订阅标识。
    pub(super) subscription_id: CaptureSubscriptionId,
    /// 任一 surface 到帧时触发的异步唤醒器。
    pub(super) notify: Arc<Notify>,
}

/// 发往唯一主机线程的串行命令。
pub(super) enum CaptureCommand {
    /// 创建单 HWND 捕获订阅。
    Open {
        window: WindowIdentity,
        policy: CapturePolicy,
        reply: oneshot::Sender<Result<OpenedCapture, VisionError>>,
    },
    /// 轮询订阅的独立 surface。
    Poll {
        subscription_id: CaptureSubscriptionId,
        frame_id: FrameId,
        deadline: Instant,
        timeout: Duration,
        reply: oneshot::Sender<Result<Option<Arc<CapturedFrame>>, VisionError>>,
    },
    /// 刷新并读取当前窗口拓扑代数。
    CurrentTopology {
        subscription_id: CaptureSubscriptionId,
        reply: oneshot::Sender<Result<TopologyGeneration, VisionError>>,
    },
    /// 销毁单个订阅的 WGC 资源。
    Close {
        subscription_id: CaptureSubscriptionId,
    },
    /// 销毁全部捕获资源并终止主机线程。
    Shutdown { reply: SyncSender<()> },
}

/// 构造捕获主机生命周期错误。
pub(super) fn capture_host_error(message: impl Into<String>) -> VisionError {
    VisionError::CaptureUnavailable {
        message: message.into(),
    }
}

/// 主机已停止或正在停止时返回稳定错误。
fn host_stopped() -> VisionError {
    capture_host_error("application capture host is not running")
}
