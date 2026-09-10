//! 引擎额度、运行句柄与最终清理监督。
use super::{guard::guard_future, runner::Runner, state::*};
use crate::{PreparedWorkflow, Resource, ResourcePool, RunError};
use argusflow_core::{Operation, OperationOptions};
use argusflow_workflow::{ErrorKind, Values};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Semaphore, broadcast, watch};

static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
/// 单次运行的数据及宿主借用资源。
#[derive(Default)]
pub struct RunInputs {
    /// 类型必须完全满足根输入声明。
    pub values: Values,
    /// 宿主保持所有权，引擎不会关闭这些资源。
    pub resources: BTreeMap<String, Arc<dyn Resource>>,
}
/// 可并行管理隔离运行的引擎；每次运行内部串行执行。
pub struct WorkflowEngine {
    max_runs: u32,
    runs: Arc<Semaphore>,
    pool: Arc<ResourcePool>,
}
impl Default for WorkflowEngine {
    fn default() -> Self {
        Self {
            max_runs: 8,
            runs: Arc::new(Semaphore::new(8)),
            pool: Arc::new(ResourcePool::new(64)),
        }
    }
}
impl WorkflowEngine {
    /// 默认最多八个运行和六十四个自有资源。
    pub fn new() -> Self {
        Self::default()
    }
    /// 以显式额度创建引擎，失败清理资源也计入额度。
    pub fn with_limits(max_runs: usize, max_resources: usize) -> Result<Self, RunError> {
        if max_runs == 0 || max_runs > 128 || max_resources == 0 || max_resources > 4096 {
            return Err(RunError::new(ErrorKind::Limit, "引擎额度超出范围"));
        }
        Ok(Self {
            max_runs: max_runs as u32,
            runs: Arc::new(Semaphore::new(max_runs)),
            pool: Arc::new(ResourcePool::new(max_resources)),
        })
    }
    /// 验证输入后启动已经冻结的计划；返回句柄拥有取消生命周期。
    pub fn start(
        &self,
        plan: Arc<PreparedWorkflow>,
        inputs: RunInputs,
        options: RunOptions,
    ) -> Result<RunHandle, RunError> {
        options.validate()?;
        if inputs.values.keys().ne(plan.inputs.keys())
            || inputs.values.iter().any(|(name, value)| {
                !value.matches(&plan.inputs[name])
                    || value
                        .measured_bytes()
                        .is_none_or(|n| n > options.value_bytes)
            })
            || inputs.resources.keys().ne(plan.resource_inputs.keys())
            || inputs
                .resources
                .iter()
                .any(|(name, r)| r.resource_type() != plan.resource_inputs[name])
        {
            return Err(RunError::new(
                ErrorKind::Contract,
                "运行输入或资源不满足根声明",
            ));
        }
        if inputs
            .values
            .values()
            .try_fold(0usize, |sum, v| sum.checked_add(v.measured_bytes()?))
            .is_none_or(|n| n > options.data_bytes)
        {
            return Err(RunError::new(ErrorKind::Limit, "输入数据超过总预算"));
        }
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| RunError::new(ErrorKind::Contract, "start 必须运行在 Tokio runtime 内"))?;
        let permit = self
            .runs
            .clone()
            .try_acquire_owned()
            .map_err(|_| RunError::new(ErrorKind::Busy, "活动运行额度已满"))?;
        let id = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
        let operation =
            Operation::new(OperationOptions::new(options.timeout).map_err(RunError::from)?);
        let (events, _) = broadcast::channel(256);
        let (state, receiver) = watch::channel((RunStatus::Running, None));
        let initial_events = events.subscribe();
        let pool = self.pool.clone();
        let run_operation = operation.clone();
        let event_sender = events.clone();
        let sequence = Arc::new(AtomicU64::new(1));
        runtime.spawn(async move {
            let cleanup_timeout = options.cleanup_timeout;
            let runner = Runner::new(
                id,
                plan,
                inputs,
                options,
                run_operation.clone(),
                pool.clone(),
                event_sender.clone(),
                sequence.clone(),
                state.clone(),
            );
            let guarded = guard_future(async { Ok(runner.run().await) }).await;
            let _ = state.send((RunStatus::Cleaning, None));
            let (mut result, cleanup) = match guarded {
                Ok(result) => (result, Vec::new()),
                Err(error) => (
                    Err(error),
                    pool.cleanup_run(Some(id), cleanup_timeout).await,
                ),
            };
            if result.is_ok() {
                if run_operation.is_cancelled() {
                    result = Err(RunError::new(ErrorKind::Cancelled, "运行已取消"));
                } else if run_operation.remaining().is_zero() {
                    result = Err(RunError::new(ErrorKind::RunTimeout, "运行总时限已到"));
                }
            }
            for error in cleanup {
                match &mut result {
                    Err(primary) => primary.secondary.push(error),
                    Ok(_) => result = Err(error),
                }
            }
            let (status, outputs, error) = match result {
                Ok(outputs) => (RunStatus::Completed, outputs, None),
                Err(error) => (
                    match error.kind {
                        ErrorKind::Cancelled => RunStatus::Cancelled,
                        ErrorKind::Timeout | ErrorKind::RunTimeout => RunStatus::TimedOut,
                        _ => RunStatus::Failed,
                    },
                    Values::new(),
                    Some(error),
                ),
            };
            let final_result = Arc::new(RunResult {
                run_id: id,
                status,
                outputs,
                error,
            });
            let _ = event_sender.send(ExecutionEvent {
                run_id: id,
                sequence: sequence.fetch_add(1, Ordering::Relaxed),
                path: Vec::new(),
                kind: EventKind::RunFinished,
            });
            // 最终状态可见时，下一次运行和遗留清理必须已经可以申请额度。
            drop(permit);
            let _ = state.send((status, Some(final_result)));
        });
        Ok(RunHandle {
            id,
            operation,
            state: receiver,
            events: initial_events,
            subscribed: false,
        })
    }
    /// 当前自有资源数量，含回收失败的资源。
    pub async fn retained_resources(&self) -> usize {
        self.pool.count().await
    }
    /// 仅在没有活动运行时重试遗留资源清理，防止关闭活跃流程对象。
    pub async fn retry_cleanup(&self, timeout: Duration) -> Result<(), Vec<RunError>> {
        let _all = self
            .runs
            .clone()
            .try_acquire_many_owned(self.max_runs)
            .map_err(|_| {
                vec![RunError::new(
                    ErrorKind::Busy,
                    "仍有活动运行，不能重试全局清理",
                )]
            })?;
        let errors = self.pool.cleanup_run(None, timeout).await;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// 活跃运行的唯一控制句柄，丢弃时请求取消；不会阻塞析构。
pub struct RunHandle {
    id: u64,
    operation: Operation,
    state: watch::Receiver<(RunStatus, Option<Arc<RunResult>>)>,
    events: broadcast::Receiver<ExecutionEvent>,
    subscribed: bool,
}
impl RunHandle {
    /// 稳定运行 ID。
    pub fn id(&self) -> u64 {
        self.id
    }
    /// 取消根运行，执行器继续有界收尾。
    pub fn cancel(&self) {
        self.operation.cancel();
    }
    /// 当前状态，不依赖事件是否被消费。
    pub fn status(&self) -> RunStatus {
        self.state.borrow().0
    }
    /// 第一份订阅从 start 建立，之后的订阅从当前时刻建立。
    pub fn subscribe(&mut self) -> EventSubscription {
        let receiver = if self.subscribed {
            self.events.resubscribe()
        } else {
            self.subscribed = true;
            let next = self.events.resubscribe();
            std::mem::replace(&mut self.events, next)
        };
        EventSubscription { receiver }
    }
    /// 若已经结束，直接读取最终结果。
    pub fn result(&self) -> Option<Arc<RunResult>> {
        self.state.borrow().1.clone()
    }
    /// 等待包括清理在内的最终状态。
    pub async fn wait(&mut self) -> Result<Arc<RunResult>, RunError> {
        loop {
            if let Some(result) = self.result() {
                return Ok(result);
            }
            self.state
                .changed()
                .await
                .map_err(|_| RunError::new(ErrorKind::Contract, "运行监督任务意外退出"))?;
        }
    }
}
impl Drop for RunHandle {
    fn drop(&mut self) {
        if self.result().is_none() {
            self.operation.cancel();
        }
    }
}
/// 有界事件接收器，显式报告消费缺口。
pub struct EventSubscription {
    receiver: broadcast::Receiver<ExecutionEvent>,
}
impl EventSubscription {
    /// 等待事件、缺口或关闭通知。
    pub async fn recv(&mut self) -> EventRead {
        match self.receiver.recv().await {
            Ok(event) => EventRead::Event(event),
            Err(broadcast::error::RecvError::Lagged(n)) => EventRead::Gap(n),
            Err(broadcast::error::RecvError::Closed) => EventRead::Closed,
        }
    }
}
