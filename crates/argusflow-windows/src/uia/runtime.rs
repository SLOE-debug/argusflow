//! 可恢复 UIA worker generation、typed request channel 与只读 runtime health。

use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use argusflow_agent::{EvidenceBundle, EvidenceCaptureError, EvidenceCaptureRequest};
use argusflow_core::{ActionOutcome, AutomationError, BackendKind, EntityObservation};
use tokio::sync::oneshot;

use super::{
    budget::UiaExecutionBudget,
    plan::UiaPreparedPlan,
    runtime_health::{UiaRuntimeHealth, UiaRuntimeState},
    runtime_worker::UiaWorkerGeneration,
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

/// 一次 worker 往返中执行的有序 selector 计划集合。
#[derive(Debug)]
pub(crate) struct UiaObserveRequest {
    /// prepare 阶段冻结的窗口身份。
    pub(crate) window: PreparedWindowTarget,
    /// 每个 AQL selector 的有序备选计划；首个非空结果即为该 selector 的事实。
    pub(crate) plans: Vec<Vec<super::plan::UiaQueryPlan>>,
}

/// 不携带 COM interface、只绑定 prepared 状态的 UIA evidence 请求。
#[derive(Debug)]
pub(crate) struct UiaEvidenceRequest {
    /// prepare 阶段冻结的窗口身份。
    pub(crate) window: PreparedWindowTarget,
    /// 与失败 candidate 完全相同的查询和动作计划。
    pub(crate) plan: UiaPreparedPlan,
    /// 规范化查询。
    pub(crate) query: String,
    /// PreparedPlan 分类后的通用采集请求。
    pub(crate) capture: EvidenceCaptureRequest,
}

/// 应用生命周期持有并在 deadline 后受控替换的 UIA worker generation。
pub struct UiaRuntime {
    /// 当前唯一可接收新请求的 worker generation。
    pub(super) worker: Mutex<UiaWorkerGeneration>,
    /// Backend 与 context provider 共享的 health。
    pub(super) health: Arc<UiaRuntimeHealth>,
    /// provider timeout、请求资源限制与恢复上限。
    config: UiaRuntimeConfig,
}

impl UiaRuntime {
    /// 启动第一代名为 `argusflow-uia-0` 的专用 MTA worker。
    pub fn start() -> Self {
        let config = UiaRuntimeConfig::default();
        Self::start_with_config(config)
    }

    /// 使用完整内部配置启动第一代 worker。
    fn start_with_config(config: UiaRuntimeConfig) -> Self {
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

    /// 异步提交一批 UIA 观察计划，保持同一 worker 与窗口身份。
    pub(crate) async fn observe(
        &self,
        request: UiaObserveRequest,
    ) -> Result<Vec<EntityObservation>, AutomationError> {
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
            if worker
                .send_observe(request, budget, response_sender)
                .is_err()
            {
                return Err(unavailable(
                    "UI Automation observation channel is closed".to_owned(),
                ));
            }
            generation
        };
        match tokio::time::timeout(self.config.execution_timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.recover_worker(generation);
                Err(unavailable(
                    "UI Automation worker stopped during observation".to_owned(),
                ))
            }
            Err(_) => {
                self.recover_worker(generation);
                Err(unavailable(
                    "UI Automation observation exceeded its deadline".to_owned(),
                ))
            }
        }
    }

    /// 在同一 UIA worker apartment 中采集冻结 candidate 的普通 Rust snapshot。
    pub(crate) async fn capture(
        &self,
        request: UiaEvidenceRequest,
    ) -> Result<EvidenceBundle, EvidenceCaptureError> {
        if !self.health.is_ready() {
            return Err(evidence_unavailable(runtime_state_message(
                self.health.snapshot(),
            )));
        }
        let capture_timeout = request.capture.budget.deadline;
        let (response_sender, response_receiver) = oneshot::channel();
        let generation = {
            let worker = self
                .worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let generation = worker.generation();
            if !self.health.is_ready_generation(generation) {
                return Err(evidence_unavailable(runtime_state_message(
                    self.health.snapshot(),
                )));
            }
            if worker.send_capture(request, response_sender).is_err() {
                return Err(evidence_unavailable(
                    "UI Automation worker evidence channel is closed".to_owned(),
                ));
            }
            generation
        };

        match tokio::time::timeout(capture_timeout, response_receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => {
                self.recover_worker(generation);
                Err(evidence_unavailable(
                    "UI Automation worker stopped before returning evidence".to_owned(),
                ))
            }
            Err(_) => {
                self.recover_worker(generation);
                Err(EvidenceCaptureError::DeadlineExceeded)
            }
        }
    }

    /// 只允许触发故障的当前 generation 启动下一代，避免并发 timeout 重复恢复。
    pub(super) fn recover_worker(&self, expected_generation: u64) {
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
    /// 单次请求允许由进程查询返回或通过 RawView TreeWalker 访问的节点总数。
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

/// 创建不会覆盖主 AutomationError 的采集不可用错误。
fn evidence_unavailable(message: String) -> EvidenceCaptureError {
    EvidenceCaptureError::SourceUnavailable { message }
}
