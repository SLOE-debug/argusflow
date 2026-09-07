//! UIA worker generation 的生命周期快照与原子状态转换。

use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

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
