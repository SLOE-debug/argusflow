//! 专用 MTA COM worker、typed request channel 与只读 runtime health。

use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
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
    plan::UiaPreparedPlan,
};

/// prepare 阶段冻结、execute 阶段重新校验的窗口身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreparedWindowTarget {
    /// HWND 的无符号稳定表示。
    pub(crate) handle: u64,
    /// prepare 时 HWND 所属进程。
    pub(crate) process_id: u32,
}

/// 不携带任何 COM interface 的 UIA worker 请求。
#[derive(Debug)]
pub(crate) struct UiaExecuteRequest {
    /// prepare 冻结的窗口身份。
    pub(crate) window: PreparedWindowTarget,
    /// prepare 冻结的查询、动作与联合能力计划。
    pub(crate) plan: UiaPreparedPlan,
    /// 规范化查询，仅用于公共错误复现。
    pub(crate) query: String,
}

/// UIA worker 的可观察生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaRuntimeState {
    /// worker 正在初始化 COM apartment 与 client。
    Initializing,
    /// worker 已可接收真实 UIA 请求。
    Ready,
    /// COM apartment、client 或线程创建失败。
    InitializationFailed {
        /// 可用于 Planner/日志诊断的失败原因。
        message: String,
    },
    /// 初始化成功后 worker 已退出。
    Stopped,
}

/// 可由 Backend 与 ExecutionContextProvider 共享的只读 runtime health。
#[derive(Debug)]
pub struct UiaRuntimeHealth {
    /// 原子状态码使高频 Planner snapshot 无需持有字符串锁。
    state: AtomicU8,
    /// 初始化失败或请求 deadline 熔断时保存的最新诊断文本。
    failure: Mutex<Option<String>>,
}

impl UiaRuntimeHealth {
    /// 返回当前 worker 状态的不可变快照。
    pub fn snapshot(&self) -> UiaRuntimeState {
        match self.state.load(Ordering::Acquire) {
            HEALTH_READY => UiaRuntimeState::Ready,
            HEALTH_FAILED => UiaRuntimeState::InitializationFailed {
                message: self
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone()
                    .unwrap_or_else(|| "UI Automation initialization failed".to_owned()),
            },
            HEALTH_STOPPED => UiaRuntimeState::Stopped,
            _ => UiaRuntimeState::Initializing,
        }
    }

    /// 判断 runtime 是否可进入 Ready candidate。
    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == HEALTH_READY
    }

    /// 仅允许仍处于初始化状态的 worker 标记成功，避免启动 deadline 后恢复 Ready。
    fn mark_ready(&self) -> bool {
        self.state
            .compare_exchange(
                HEALTH_INITIALIZING,
                HEALTH_READY,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// 保存稳定失败原因并标记初始化失败。
    fn mark_failed(&self, message: String) {
        *self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(message);
        self.state.store(HEALTH_FAILED, Ordering::Release);
    }

    /// 标记已经成功启动过的 worker 停止。
    fn mark_stopped(&self) {
        self.state.store(HEALTH_STOPPED, Ordering::Release);
    }
}

impl Default for UiaRuntimeHealth {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(HEALTH_INITIALIZING),
            failure: Mutex::new(None),
        }
    }
}

/// 应用生命周期持有的唯一 UIA worker handle。
pub struct UiaRuntime {
    /// 只发送 ArgusFlow 自有 Send 数据的标准线程 channel。
    sender: Sender<UiaWorkerMessage>,
    /// Backend 与 context provider 共享的 health。
    health: Arc<UiaRuntimeHealth>,
    /// worker handle；只对已经退出的线程执行 join，避免第三方 provider 卡住 Drop。
    worker: Mutex<Option<JoinHandle<()>>>,
    /// provider timeout 与 ArgusFlow 请求资源限制。
    config: UiaRuntimeConfig,
}

impl UiaRuntime {
    /// 启动名为 `argusflow-uia` 的专用 MTA worker。
    pub fn start() -> Self {
        let config = UiaRuntimeConfig::default();
        let (sender, receiver) = mpsc::channel();
        let (initialized_sender, initialized_receiver) = mpsc::sync_channel(0);
        let health = Arc::new(UiaRuntimeHealth::default());
        let worker_health = health.clone();
        let worker = thread::Builder::new()
            .name("argusflow-uia".to_owned())
            .spawn(move || worker_main(receiver, worker_health, initialized_sender, config));

        let worker = match worker {
            Ok(worker) => {
                if initialized_receiver
                    .recv_timeout(config.connection_timeout)
                    .is_err()
                    && matches!(health.snapshot(), UiaRuntimeState::Initializing)
                {
                    health.mark_failed(
                        "UI Automation worker did not initialize within the connection timeout"
                            .to_owned(),
                    );
                }
                Some(worker)
            }
            Err(error) => {
                health.mark_failed(format!("failed to spawn UI Automation worker: {error}"));
                None
            }
        };
        Self {
            sender,
            health,
            worker: Mutex::new(worker),
            config,
        }
    }

    /// 返回可与上下文提供器共享的 health handle。
    pub fn health(&self) -> Arc<UiaRuntimeHealth> {
        self.health.clone()
    }

    /// 异步提交请求，COM 同步调用始终留在专用 OS 线程。
    pub(crate) async fn execute(
        &self,
        request: UiaExecuteRequest,
    ) -> Result<ActionOutcome, AutomationError> {
        if !self.health.is_ready() {
            return Err(AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: runtime_state_message(self.health.snapshot()),
            });
        }
        let (response_sender, response_receiver) = oneshot::channel();
        let budget = UiaExecutionBudget::new(
            self.config.execution_timeout,
            self.config.max_candidates,
            self.config.max_relation_roots,
        );
        self.sender
            .send(UiaWorkerMessage::Execute {
                request,
                budget,
                response: response_sender,
            })
            .map_err(|_| AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: "UI Automation worker request channel is closed".to_owned(),
            })?;
        match tokio::time::timeout(self.config.execution_timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AutomationError::BackendUnavailable {
                backend: BackendKind::WindowsUia,
                message: "UI Automation worker stopped before returning a result".to_owned(),
            }),
            Err(_) => {
                self.health.mark_failed(
                    "UI Automation request exceeded the ArgusFlow execution deadline".to_owned(),
                );
                let _ = self.sender.send(UiaWorkerMessage::Shutdown);
                Err(AutomationError::BackendUnavailable {
                    backend: BackendKind::WindowsUia,
                    message: "UI Automation request exceeded the ArgusFlow execution deadline"
                        .to_owned(),
                })
            }
        }
    }
}

impl fmt::Debug for UiaRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UiaRuntime")
            .field("state", &self.health.snapshot())
            .finish_non_exhaustive()
    }
}

impl Drop for UiaRuntime {
    fn drop(&mut self) {
        let _ = self.sender.send(UiaWorkerMessage::Shutdown);
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            // 不对可能仍卡在第三方 provider 的线程执行无上限 join；已退出线程仍完成回收。
            if worker.is_finished() {
                let _ = worker.join();
            }
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

/// 初始化 apartment/client，并同步处理全部 UIA 请求。
fn worker_main(
    receiver: Receiver<UiaWorkerMessage>,
    health: Arc<UiaRuntimeHealth>,
    initialized: mpsc::SyncSender<()>,
    config: UiaRuntimeConfig,
) {
    let apartment = match ComApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            health.mark_failed(format!("CoInitializeEx failed: {error}"));
            let _ = initialized.send(());
            return;
        }
    };
    // SAFETY: COM 已在当前 worker 初始化；client 创建后不离开这个线程。
    let automation: IUIAutomation2 =
        match unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) } {
            Ok(automation) => automation,
            Err(error) => {
                health.mark_failed(format!("CUIAutomation8 creation failed: {error}"));
                let _ = initialized.send(());
                return;
            }
        };
    if let Err(error) = configure_provider_timeouts(&automation, config) {
        health.mark_failed(error.to_string());
        let _ = initialized.send(());
        return;
    }
    if !health.mark_ready() {
        let _ = initialized.send(());
        return;
    }
    let _ = initialized.send(());
    let _health_guard = WorkerHealthGuard(health.clone());
    let executor = UiaExecutor::new(&automation);
    while let Ok(message) = receiver.recv() {
        match message {
            UiaWorkerMessage::Execute {
                request,
                budget,
                response,
            } => {
                let result = if health.is_ready() {
                    executor.execute(request, budget)
                } else {
                    Err(AutomationError::BackendUnavailable {
                        backend: BackendKind::WindowsUia,
                        message: runtime_state_message(health.snapshot()),
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

/// 保证初始化成功后的正常退出或 panic 都反映为 Stopped。
struct WorkerHealthGuard(Arc<UiaRuntimeHealth>);

impl Drop for WorkerHealthGuard {
    fn drop(&mut self) {
        self.0.mark_stopped();
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

/// 把 health snapshot 转成执行边界的稳定诊断。
fn runtime_state_message(state: UiaRuntimeState) -> String {
    match state {
        UiaRuntimeState::Initializing => "UI Automation worker is still initializing".to_owned(),
        UiaRuntimeState::Ready => "UI Automation worker is ready".to_owned(),
        UiaRuntimeState::InitializationFailed { message } => message,
        UiaRuntimeState::Stopped => "UI Automation worker has stopped".to_owned(),
    }
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
fn duration_millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis())
        .unwrap_or(u32::MAX)
        .max(1)
}

/// UIA runtime 的稳定 provider timeout 与单次请求资源限制。
#[derive(Debug, Clone, Copy)]
struct UiaRuntimeConfig {
    /// provider 建立连接的最长时间。
    connection_timeout: Duration,
    /// 单个跨进程 UIA 调用的最长事务时间。
    transaction_timeout: Duration,
    /// 包含 worker 排队时间的 ArgusFlow 请求总时限。
    execution_timeout: Duration,
    /// 单次请求允许读取的 provider 候选总数。
    max_candidates: usize,
    /// 单次请求允许展开的关系根总数。
    max_relation_roots: usize,
}

impl Default for UiaRuntimeConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(2),
            transaction_timeout: Duration::from_secs(20),
            execution_timeout: Duration::from_secs(25),
            max_candidates: 10_000,
            max_relation_roots: 256,
        }
    }
}

/// health 原子状态码。
const HEALTH_INITIALIZING: u8 = 0;
/// health 原子状态码。
const HEALTH_READY: u8 = 1;
/// health 原子状态码。
const HEALTH_FAILED: u8 = 2;
/// health 原子状态码。
const HEALTH_STOPPED: u8 = 3;
