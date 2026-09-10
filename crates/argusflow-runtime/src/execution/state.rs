//! 可查询的最终状态独立于可丢失的有界观测事件。
use crate::{ExecutionLocation, RunError};
use argusflow_workflow::Values;
use std::time::Duration;

/// 所有运行预算必须是有限值。
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// 总时限，默认三十分钟。
    pub timeout: Duration,
    /// 含循环体的节点执行总数，默认十万。
    pub max_steps: u64,
    /// 未指定轮数时的循环上限，默认一万。
    pub max_iterations: u32,
    /// 活跃帧上限，默认 64。
    pub max_depth: usize,
    /// 每次节点表达式计算最多操作数。
    pub expression_steps: usize,
    /// 单个值的最大估计字节数，默认 1 MiB。
    pub value_bytes: usize,
    /// 活跃数据及节点输出的总估计字节数，默认 16 MiB。
    pub data_bytes: usize,
    /// 取消或离开作用域后的清理时限，默认五秒。
    pub cleanup_timeout: Duration,
}
impl Default for RunOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1800),
            max_steps: 100_000,
            max_iterations: 10_000,
            max_depth: 64,
            expression_steps: 10_000,
            value_bytes: 1024 * 1024,
            data_bytes: 16 * 1024 * 1024,
            cleanup_timeout: Duration::from_secs(5),
        }
    }
}
impl RunOptions {
    pub(super) fn validate(&self) -> Result<(), RunError> {
        if self.timeout.is_zero()
            || self.timeout > Duration::from_secs(86_400)
            || self.max_steps == 0
            || self.max_steps > 10_000_000
            || self.max_iterations == 0
            || self.max_iterations > 100_000
            || self.max_depth == 0
            || self.max_depth > 64
            || self.expression_steps == 0
            || self.expression_steps > 1_000_000
            || self.value_bytes == 0
            || self.value_bytes > 64 * 1024 * 1024
            || self.data_bytes < self.value_bytes
            || self.data_bytes > 256 * 1024 * 1024
            || self.cleanup_timeout.is_zero()
            || self.cleanup_timeout > Duration::from_secs(60)
        {
            return Err(RunError::new(
                argusflow_workflow::ErrorKind::Limit,
                "运行预算超出支持范围",
            ));
        }
        Ok(())
    }
}
/// 运行的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// 正在执行。
    Running,
    /// 正在完成最终清理。
    Cleaning,
    /// 执行与清理都成功。
    Completed,
    /// 执行或清理失败。
    Failed,
    /// 用户请求取消。
    Cancelled,
    /// 总时限或未处理的局部时限耗尽。
    TimedOut,
}
/// 最终结果可以独立于事件接收器查询。
#[derive(Debug, Clone)]
pub struct RunResult {
    /// 稳定运行 ID。
    pub run_id: u64,
    /// 最终状态。
    pub status: RunStatus,
    /// 成功返回的数据，失败时为空。
    pub outputs: Values,
    /// 含原始失败与附加清理错误。
    pub error: Option<RunError>,
}
/// 不包含业务数据的事件种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// 根运行开始。
    RunStarted,
    /// 建立局部激活帧。
    ScopeEntered,
    /// 节点开始。
    NodeStarted,
    /// 节点结果已发布。
    NodeCompleted,
    /// 节点失败或控制结构传播失败。
    NodeFailed,
    /// 下一次安全重试。
    Retrying {
        /// 从一开始的尝试次数。
        attempt: u32,
    },
    /// 帧已经退出，清理错误另由最终结果保留。
    ScopeExited,
    /// 运行结束。
    RunFinished,
}
/// 单调有序的有限观测事件。
#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    /// 运行关联。
    pub run_id: u64,
    /// 本次运行内的连续事件序号。
    pub sequence: u64,
    /// 作用域实例、节点执行序号与真实调用路径。
    pub path: Vec<ExecutionLocation>,
    /// 发生的事件。
    pub kind: EventKind,
}
/// 有界事件订阅的显式读取结果。
#[derive(Debug, Clone)]
pub enum EventRead {
    /// 一个完整事件。
    Event(ExecutionEvent),
    /// 消费过慢，已丢失指定数量事件。
    Gap(u64),
    /// 运行事件源已关闭。
    Closed,
}
