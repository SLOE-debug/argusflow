//! 单个 UIA MTA worker generation 的线程、COM apartment 与消息循环。

use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use argusflow_core::{ActionOutcome, AutomationError, BackendKind};
use tokio::sync::oneshot;
use windows::Win32::{
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    },
    UI::Accessibility::{CUIAutomation8, IUIAutomation2},
};

use super::{
    budget::UiaExecutionBudget,
    error::{UiaError, UiaOperation},
    executor::UiaExecutor,
    runtime::{UiaExecuteRequest, UiaRuntimeConfig, UiaRuntimeHealth},
};

/// 当前 runtime 唯一接收新请求的 worker generation。
pub(super) struct UiaWorkerGeneration {
    /// 单调递增的恢复代号。
    generation: u64,
    /// 只发送 ArgusFlow 自有 Send 数据的标准线程 channel。
    sender: Sender<UiaWorkerMessage>,
    /// 线程句柄；卡在第三方 provider 时允许安全分离。
    thread: Option<JoinHandle<()>>,
}

impl UiaWorkerGeneration {
    /// 创建、初始化并最多等待 connection timeout 的新 worker generation。
    pub(super) fn start(
        generation: u64,
        health: Arc<UiaRuntimeHealth>,
        config: UiaRuntimeConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        let (initialized_sender, initialized_receiver) = mpsc::sync_channel(0);
        let worker_health = health.clone();
        let thread = thread::Builder::new()
            .name(format!("argusflow-uia-{generation}"))
            .spawn(move || {
                worker_main(
                    receiver,
                    worker_health,
                    initialized_sender,
                    config,
                    generation,
                );
            });
        let thread = match thread {
            Ok(thread) => {
                if initialized_receiver
                    .recv_timeout(config.connection_timeout)
                    .is_err()
                {
                    health.mark_failed(
                        generation,
                        "UI Automation worker did not initialize within the connection timeout"
                            .to_owned(),
                    );
                }
                Some(thread)
            }
            Err(error) => {
                health.mark_failed(
                    generation,
                    format!("failed to spawn UI Automation worker: {error}"),
                );
                None
            }
        };
        Self {
            generation,
            sender,
            thread,
        }
    }

    /// 返回当前 worker 的稳定 generation。
    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    /// 提交一次不含 COM interface 的冻结请求。
    pub(super) fn send(
        &self,
        request: UiaExecuteRequest,
        budget: UiaExecutionBudget,
        response: oneshot::Sender<Result<ActionOutcome, AutomationError>>,
    ) -> Result<(), ()> {
        self.sender
            .send(UiaWorkerMessage::Execute {
                request,
                budget,
                response,
            })
            .map_err(|_| ())
    }

    /// 请求在创建 apartment 的线程退出；仍卡住的 provider 线程不会阻塞调用方。
    pub(super) fn shutdown(&mut self) {
        let _ = self.sender.send(UiaWorkerMessage::Shutdown);
        if let Some(thread) = self.thread.take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
    }
}

/// worker 可处理的封闭消息集合。
enum UiaWorkerMessage {
    /// 执行冻结请求并通过 oneshot 返回公共结果。
    Execute {
        /// 不含 COM interface 的请求。
        request: UiaExecuteRequest,
        /// 包含 channel 排队时间的请求预算。
        budget: UiaExecutionBudget,
        /// 唯一响应 channel。
        response: oneshot::Sender<Result<ActionOutcome, AutomationError>>,
    },
    /// 在创建 apartment 的同一线程退出。
    Shutdown,
}

/// 初始化 apartment/client，并同步处理当前 generation 的全部 UIA 请求。
fn worker_main(
    receiver: Receiver<UiaWorkerMessage>,
    health: Arc<UiaRuntimeHealth>,
    initialized: mpsc::SyncSender<()>,
    config: UiaRuntimeConfig,
    generation: u64,
) {
    let apartment = match ComApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            health.mark_failed(generation, format!("CoInitializeEx failed: {error}"));
            let _ = initialized.send(());
            return;
        }
    };
    // SAFETY: COM 已在当前 worker 初始化；client 创建后不离开这个线程。
    let automation: IUIAutomation2 =
        match unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) } {
            Ok(automation) => automation,
            Err(error) => {
                health.mark_failed(
                    generation,
                    format!("CUIAutomation8 creation failed: {error}"),
                );
                let _ = initialized.send(());
                return;
            }
        };
    if let Err(error) = configure_provider_timeouts(&automation, config) {
        health.mark_failed(generation, error.to_string());
        let _ = initialized.send(());
        return;
    }
    if !health.mark_ready(generation) {
        let _ = initialized.send(());
        return;
    }
    let _ = initialized.send(());
    let _health_guard = WorkerHealthGuard {
        health: health.clone(),
        generation,
    };
    let executor = UiaExecutor::new(&automation);
    while let Ok(message) = receiver.recv() {
        match message {
            UiaWorkerMessage::Execute {
                request,
                budget,
                response,
            } => {
                let result = if health.is_ready_generation(generation) {
                    executor.execute(request, budget)
                } else {
                    Err(AutomationError::BackendUnavailable {
                        backend: BackendKind::WindowsUia,
                        message: "UI Automation worker generation was superseded".to_owned(),
                    })
                };
                let _ = response.send(result);
            }
            UiaWorkerMessage::Shutdown => break,
        }
    }
    drop(executor);
    drop(automation);
    drop(apartment);
}

/// 显式设置 UIA provider 自带的连接与事务超时，不依赖系统默认值。
fn configure_provider_timeouts(
    automation: &IUIAutomation2,
    config: UiaRuntimeConfig,
) -> Result<(), UiaError> {
    let connection_timeout = duration_millis(config.connection_timeout);
    let transaction_timeout = duration_millis(config.transaction_timeout);
    // SAFETY: automation client 在当前 MTA worker 创建，配置调用不会离开该 apartment。
    unsafe { automation.SetConnectionTimeout(connection_timeout) }
        .map_err(|source| UiaError::from_native(UiaOperation::ConfigureTimeouts, source))?;
    // SAFETY: automation client 仍由当前 worker 独占，毫秒值已收窄为 API 接受的 u32。
    unsafe { automation.SetTransactionTimeout(transaction_timeout) }
        .map_err(|source| UiaError::from_native(UiaOperation::ConfigureTimeouts, source))?;
    Ok(())
}

/// 把受控 runtime Duration 转换为 IUIAutomation2 使用的非零毫秒值。
fn duration_millis(duration: std::time::Duration) -> u32 {
    u32::try_from(duration.as_millis())
        .unwrap_or(u32::MAX)
        .max(1)
}

/// 防止旧 worker 退出覆盖新 generation 的 health。
struct WorkerHealthGuard {
    /// generation-aware 共享状态。
    health: Arc<UiaRuntimeHealth>,
    /// 当前 worker 的稳定 generation。
    generation: u64,
}

impl Drop for WorkerHealthGuard {
    fn drop(&mut self) {
        self.health.mark_stopped(self.generation);
    }
}

/// 只允许在初始化线程调用 CoUninitialize 的 RAII guard。
struct ComApartment;

impl ComApartment {
    /// 初始化多线程 COM apartment。
    fn initialize() -> windows::core::Result<Self> {
        // SAFETY: 每个 UIA worker 只调用一次，并由 ComApartment::drop 在同线程配对。
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: guard 只在成功调用 CoInitializeEx 的 worker 线程构造和销毁。
        unsafe { CoUninitialize() };
    }
}
