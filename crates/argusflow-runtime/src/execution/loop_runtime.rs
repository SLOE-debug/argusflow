//! While 激活帧和显式迭代预算。

use std::time::{Duration, Instant};

use argusflow_core::{ExecutionLoopFrame, ExecutionStructureFrame};

/// 执行栈中的一个活动 While 容器。
#[derive(Debug)]
pub(super) struct LoopFrame {
    /// 容器所在父作用域。
    pub(super) parent_scope_id: String,
    /// 父作用域中的容器节点 ID。
    pub(super) container_node_id: String,
    /// 每轮执行的子作用域 ID。
    pub(super) body_scope_id: String,
    /// 单次激活最多轮次数。
    pub(super) max_iterations: u32,
    /// 激活总时长预算。
    timeout: Duration,
    /// 第二轮起的等待间隔。
    interval: Duration,
    /// 激活开始的单调时钟。
    started_at: Instant,
    /// 已经开始的轮次数。
    iterations: u32,
}

impl LoopFrame {
    /// 建立一份全新的循环激活；内层循环每次重新进入都会得到独立帧。
    pub(super) fn new(
        parent_scope_id: String,
        container_node_id: String,
        body_scope_id: String,
        max_iterations: u32,
        timeout_ms: u64,
        interval_ms: u64,
    ) -> Self {
        Self {
            parent_scope_id,
            container_node_id,
            body_scope_id,
            max_iterations,
            timeout: Duration::from_millis(timeout_ms),
            interval: Duration::from_millis(interval_ms),
            started_at: Instant::now(),
            iterations: 0,
        }
    }

    /// 等待配置间隔并尝试进入下一轮；首次激活保证至少执行一次。
    pub(super) async fn begin_next_iteration(&mut self) -> Option<u32> {
        if self.iterations > 0 && !self.interval.is_zero() {
            tokio::time::sleep(self.interval).await;
        }
        if self.iterations > 0
            && (self.iterations >= self.max_iterations || self.started_at.elapsed() >= self.timeout)
        {
            return None;
        }
        self.iterations += 1;
        Some(self.iterations)
    }

    /// 返回已经开始的轮次数。
    pub(super) const fn iterations(&self) -> u32 {
        self.iterations
    }

    /// 转换为事件公开的无业务数据结构路径帧。
    fn event_frame(&self) -> ExecutionStructureFrame {
        ExecutionStructureFrame::Loop(ExecutionLoopFrame {
            container_node_id: self.container_node_id.clone(),
            scope_id: self.body_scope_id.clone(),
            iteration: self.iterations,
        })
    }
}

/// 把活动 While 栈附加到一条待投递事件。
pub(super) fn append_loop_path(event: &mut argusflow_core::ExecutionEvent, frames: &[LoopFrame]) {
    event
        .structure_path
        .extend(frames.iter().map(LoopFrame::event_frame));
}
