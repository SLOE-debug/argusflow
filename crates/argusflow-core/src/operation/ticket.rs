//! 不依赖异步运行时的截止时间与协作取消票据。

use crate::{Effect, Failure, FailureKind};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// 仅用于进程内诊断关联，不参与资源身份。
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

/// 单次调用的总时限。
#[derive(Debug, Clone, Copy)]
pub struct OperationOptions {
    timeout: Duration,
}

impl OperationOptions {
    /// 创建有限、非零的时限，最长一天。
    pub fn new(timeout: Duration) -> Result<Self, Failure> {
        if timeout.is_zero() || timeout > Duration::from_secs(86_400) {
            return Err(Failure::new(
                FailureKind::InvalidInput,
                "options",
                "时限必须在 0 到 24 小时之间",
            ));
        }
        Ok(Self { timeout })
    }
    /// 包含排队和执行的最长时间。
    pub fn timeout(self) -> Duration {
        self.timeout
    }
}

impl Default for OperationOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Default)]
struct Signals {
    cancelled: AtomicBool,
    effect: AtomicBool,
}

/// 同一次请求在队列与执行器间传递的截止时间、取消信号及副作用标记。
#[derive(Debug, Clone)]
pub struct Operation {
    id: u64,
    deadline: Instant,
    signals: Arc<Signals>,
}

impl Operation {
    /// 在调用入口创建，之后不得为了重试重新计算截止时间。
    pub fn new(options: OperationOptions) -> Self {
        Self {
            id: NEXT_OPERATION.fetch_add(1, Ordering::Relaxed),
            deadline: Instant::now() + options.timeout(),
            signals: Arc::default(),
        }
    }
    /// 返回请求关联编号。
    pub fn id(&self) -> u64 {
        self.id
    }
    /// 返回包含队列等待的绝对截止时间。
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// 返回剩余预算，过期时为零。
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    /// 请求协作取消，不声称原生调用已退出。
    pub fn cancel(&self) {
        self.signals.cancelled.store(true, Ordering::Release);
    }
    /// 返回是否已经请求取消。
    pub fn is_cancelled(&self) -> bool {
        self.signals.cancelled.load(Ordering::Acquire)
    }
    /// 返回当前副作用的不确定性。
    pub fn effect(&self) -> Effect {
        if self.signals.effect.load(Ordering::Acquire) {
            Effect::Unconfirmed
        } else {
            Effect::None
        }
    }
    /// 为错误附加执行上下文。
    pub fn contextualize<E: Into<Failure> + From<Failure>>(&self, failure: E) -> E {
        failure.into().with_context(self.id, self.effect()).into()
    }
    /// 检查下一阶段是否仍可执行。
    pub fn check(&self, stage: &'static str) -> Result<(), Failure> {
        let failure = if self.remaining().is_zero() {
            Some(Failure::new(
                FailureKind::Timeout,
                stage,
                "操作总截止时间已到",
            ))
        } else if self.is_cancelled() {
            Some(Failure::new(FailureKind::Cancelled, stage, "操作已取消"))
        } else {
            None
        };
        failure.map_or(Ok(()), |failure| Err(self.contextualize(failure)))
    }
    /// 紧邻外部副作用调用前执行；后续错误不允许自动重放操作。
    pub fn begin_effect(&self, stage: &'static str) -> Result<(), Failure> {
        self.check(stage)?;
        self.signals.effect.store(true, Ordering::Release);
        self.check(stage)
    }
    /// 把调用 Future 的丢弃转换为共享协作取消。
    pub fn cancel_on_drop(&self) -> CancelOnDrop {
        CancelOnDrop {
            operation: self.clone(),
            armed: true,
        }
    }
}

/// 调用方持有的取消守卫；完成响应校验后解除。
pub struct CancelOnDrop {
    operation: Operation,
    armed: bool,
}
impl CancelOnDrop {
    /// 完成请求后解除自动取消。
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.operation.cancel();
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-core/unit/operation.rs"]
mod tests;
