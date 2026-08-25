//! 可恢复 UIA worker generation、typed request channel 与只读 runtime health。

use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use argusflow_core::{ActionOutcome, AutomationError, BackendKind};
use tokio::sync::oneshot;

use super::{
    budget::UiaExecutionBudget, plan::UiaPreparedPlan, runtime_worker::UiaWorkerGeneration,
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
    /// COM apartment、client、线程或受控恢复失败。
    InitializationFailed {
        /// 可用于 Planner/日志诊断的失败原因。
        message: String,
    },
    /// 初始化成功后 worker 已退出。
    Stopped,
}

/// 可由 Backend 与 ExecutionContextProvider 共享的 generation-aware runtime health。
#[derive(Debug)]
pub struct UiaRuntimeHealth {
    /// 高位保存 generation，低位保存状态，避免旧 worker 覆盖新 worker 状态。
    lifecycle: AtomicU64,
    /// 只保存与当前 generation 关联的失败诊断。
    failure: Mutex<Option<(u64, String)>>,
}

impl UiaRuntimeHealth {
    /// 返回当前 worker 状态的不可变快照。
    pub fn snapshot(&self) -> UiaRuntimeState {
        let lifecycle = self.lifecycle.load(Ordering::Acquire);
        let generation = lifecycle_generation(lifecycle);
        match lifecycle_state(lifecycle) {
            HEALTH_READY => UiaRuntimeState::Ready,
            HEALTH_FAILED => UiaRuntimeState::InitializationFailed {
                message: self
                    .failure
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                    .filter(|(failed_generation, _)| *failed_generation == generation)
                    .map(|(_, message)| message.clone())
                    .unwrap_or_else(|| "UI Automation runtime failed".to_owned()),
            },
            HEALTH_STOPPED => UiaRuntimeState::Stopped,
            _ => UiaRuntimeState::Initializing,
        }
    }

    /// 判断当前 generation 是否可进入 Ready candidate。
    pub fn is_ready(&self) -> bool {
        lifecycle_state(self.lifecycle.load(Ordering::Acquire)) == HEALTH_READY
    }

    /// 判断指定 worker generation 仍是当前 Ready 实例。
    pub(super) fn is_ready_generation(&self, generation: u64) -> bool {
        self.lifecycle.load(Ordering::Acquire) == encode_lifecycle(generation, HEALTH_READY)
    }

    /// 在启动或恢复前原子切换到新的初始化 generation。
    pub(super) fn begin_generation(&self, generation: u64) {
        self.lifecycle.store(
            encode_lifecycle(generation, HEALTH_INITIALIZING),
            Ordering::Release,
        );
        *self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    /// 仅允许当前仍在初始化的 generation 标记成功。
    pub(super) fn mark_ready(&self, generation: u64) -> bool {
        self.lifecycle
            .compare_exchange(
                encode_lifecycle(generation, HEALTH_INITIALIZING),
                encode_lifecycle(generation, HEALTH_READY),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// 保存当前 generation 的稳定失败原因；过期 worker 的写入会被忽略。
    pub(super) fn mark_failed(&self, generation: u64, message: String) {
        if self.update_generation_state(generation, HEALTH_FAILED) {
            *self
                .failure
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((generation, message));
        }
    }

    /// 标记当前 generation 已退出；过期 worker 不得覆盖恢复后的状态。
    pub(super) fn mark_stopped(&self, generation: u64) {
        let _ = self.lifecycle.compare_exchange(
            encode_lifecycle(generation, HEALTH_READY),
            encode_lifecycle(generation, HEALTH_STOPPED),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    /// 在 generation 仍匹配时替换低位状态码。
    fn update_generation_state(&self, generation: u64, state: u8) -> bool {
        let mut current = self.lifecycle.load(Ordering::Acquire);
        loop {
            if lifecycle_generation(current) != generation {
                return false;
            }
            match self.lifecycle.compare_exchange_weak(
                current,
                encode_lifecycle(generation, state),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

impl Default for UiaRuntimeHealth {
    fn default() -> Self {
        Self {
            lifecycle: AtomicU64::new(encode_lifecycle(0, HEALTH_INITIALIZING)),
            failure: Mutex::new(None),
        }
    }
}

/// 应用生命周期持有并在 deadline 后受控替换的 UIA worker generation。
pub struct UiaRuntime {
    /// 当前唯一可接收新请求的 worker generation。
    worker: Mutex<UiaWorkerGeneration>,
    /// Backend 与 context provider 共享的 health。
    health: Arc<UiaRuntimeHealth>,
    /// provider timeout、请求资源限制与恢复上限。
    config: UiaRuntimeConfig,
}

impl UiaRuntime {
    /// 启动第一代名为 `argusflow-uia-0` 的专用 MTA worker。
    pub fn start() -> Self {
        let config = UiaRuntimeConfig::default();
        let health = Arc::new(UiaRuntimeHealth::default());
        health.begin_generation(0);
        let worker = UiaWorkerGeneration::start(0, health.clone(), config);
        Self {
            worker: Mutex::new(worker),
            health,
            config,
        }
    }

    /// 返回可与上下文提供器共享的 health handle。
    pub fn health(&self) -> Arc<UiaRuntimeHealth> {
        self.health.clone()
    }

    /// 异步提交请求；单次 timeout 会触发有上限的新 generation 恢复。
    pub(crate) async fn execute(
        &self,
        request: UiaExecuteRequest,
    ) -> Result<ActionOutcome, AutomationError> {
        if !self.health.is_ready() {
            return Err(unavailable(runtime_state_message(self.health.snapshot())));
        }
        let (response_sender, response_receiver) = oneshot::channel();
        let budget = UiaExecutionBudget::new(
            self.config.execution_timeout,
            self.config.max_traversal_nodes,
            self.config.max_relation_roots,
        );
        let generation = {
            let worker = self
                .worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let generation = worker.generation();
            if !self.health.is_ready_generation(generation) {
                return Err(unavailable(runtime_state_message(self.health.snapshot())));
            }
            if worker.send(request, budget, response_sender).is_err() {
                drop(worker);
                self.recover_worker(generation);
                return Err(unavailable(
                    "UI Automation worker request channel is closed".to_owned(),
                ));
            }
            generation
        };

        match tokio::time::timeout(self.config.execution_timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.recover_worker(generation);
                Err(unavailable(
                    "UI Automation worker stopped before returning a result".to_owned(),
                ))
            }
            Err(_) => {
                self.recover_worker(generation);
                Err(unavailable(
                    "UI Automation request exceeded the ArgusFlow execution deadline".to_owned(),
                ))
            }
        }
    }

    /// 只允许触发故障的当前 generation 启动下一代，避免并发 timeout 重复恢复。
    fn recover_worker(&self, expected_generation: u64) {
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if worker.generation() != expected_generation {
            return;
        }
        worker.shutdown();
        if expected_generation >= self.config.max_recovery_attempts {
            self.health.mark_failed(
                expected_generation,
                "UI Automation runtime exhausted its recovery limit".to_owned(),
            );
            return;
        }
        let next_generation = expected_generation + 1;
        self.health.begin_generation(next_generation);
        *worker = UiaWorkerGeneration::start(next_generation, self.health.clone(), self.config);
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
        self.worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .shutdown();
    }
}

/// UIA runtime 的稳定 provider timeout、请求资源限制与恢复策略。
#[derive(Debug, Clone, Copy)]
pub(super) struct UiaRuntimeConfig {
    /// provider 建立连接的最长时间。
    pub(super) connection_timeout: Duration,
    /// 单个跨进程 UIA 调用的最长事务时间。
    pub(super) transaction_timeout: Duration,
    /// 包含 worker 排队时间的 ArgusFlow 请求总时限。
    pub(super) execution_timeout: Duration,
    /// 单次请求允许通过 RawView TreeWalker 访问的 provider 节点总数。
    pub(super) max_traversal_nodes: usize,
    /// 单次请求允许展开的关系根总数。
    pub(super) max_relation_roots: usize,
    /// 初始 generation 之后允许创建的新 worker 数量。
    pub(super) max_recovery_attempts: u64,
}

impl Default for UiaRuntimeConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(2),
            transaction_timeout: Duration::from_secs(20),
            execution_timeout: Duration::from_secs(25),
            max_traversal_nodes: 10_000,
            max_relation_roots: 256,
            max_recovery_attempts: 3,
        }
    }
}

/// 把 health snapshot 转成执行边界的稳定诊断。
fn runtime_state_message(state: UiaRuntimeState) -> String {
    match state {
        UiaRuntimeState::Initializing => {
            "UI Automation worker is initializing or recovering".to_owned()
        }
        UiaRuntimeState::Ready => "UI Automation worker is ready".to_owned(),
        UiaRuntimeState::InitializationFailed { message } => message,
        UiaRuntimeState::Stopped => "UI Automation worker has stopped".to_owned(),
    }
}

/// 创建稳定的公共后端不可用错误。
fn unavailable(message: String) -> AutomationError {
    AutomationError::BackendUnavailable {
        backend: BackendKind::WindowsUia,
        message,
    }
}

/// 把 generation 和状态编码进单个原子值。
const fn encode_lifecycle(generation: u64, state: u8) -> u64 {
    (generation << STATE_BITS) | state as u64
}

/// 从原子 lifecycle 读取 generation。
const fn lifecycle_generation(lifecycle: u64) -> u64 {
    lifecycle >> STATE_BITS
}

/// 从原子 lifecycle 读取状态码。
const fn lifecycle_state(lifecycle: u64) -> u8 {
    (lifecycle & STATE_MASK) as u8
}

/// 低位状态码占用的位数。
const STATE_BITS: u32 = 8;
/// 低位状态码掩码。
const STATE_MASK: u64 = (1 << STATE_BITS) - 1;
/// 初始化状态码。
const HEALTH_INITIALIZING: u8 = 0;
/// Ready 状态码。
const HEALTH_READY: u8 = 1;
/// 失败状态码。
const HEALTH_FAILED: u8 = 2;
/// 停止状态码。
const HEALTH_STOPPED: u8 = 3;

#[cfg(test)]
mod tests {
    use super::{UiaRuntimeHealth, UiaRuntimeState};

    /// 旧 worker 在恢复后退出时不能把新 generation 从 Ready 改成 Stopped。
    #[test]
    fn stale_generation_cannot_overwrite_recovered_health() {
        let health = UiaRuntimeHealth::default();
        health.begin_generation(0);
        assert!(health.mark_ready(0));
        health.begin_generation(1);
        assert!(health.mark_ready(1));

        health.mark_stopped(0);

        assert_eq!(health.snapshot(), UiaRuntimeState::Ready);
    }

    /// 恢复 generation 可以从旧代失败中重新进入 Ready。
    #[test]
    fn a_new_generation_recovers_from_a_previous_failure() {
        let health = UiaRuntimeHealth::default();
        health.begin_generation(0);
        health.mark_failed(0, "provider timeout".to_owned());
        assert!(matches!(
            health.snapshot(),
            UiaRuntimeState::InitializationFailed { .. }
        ));

        health.begin_generation(1);
        assert!(health.mark_ready(1));

        assert_eq!(health.snapshot(), UiaRuntimeState::Ready);
    }
}
