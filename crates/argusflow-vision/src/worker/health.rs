//! worker 状态机和有界重启策略。

use std::time::{Duration, Instant};

use crate::error::VisionError;

/// worker 连续生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDecision {
    /// 当前故障仍允许重新启动 worker。
    Restart,
    /// 已达到重启预算，必须进入 Failed。
    Fail,
}

/// worker 的重启窗口和次数上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerRestartPolicy {
    /// 一个时间窗口内允许的最大重启次数。
    pub max_restarts: u32,
    /// 统计重启次数的时间窗口。
    pub window: Duration,
}

impl Default for WorkerRestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            window: Duration::from_secs(30),
        }
    }
}

/// 对 worker crash 做 bounded restart，防止无限 crash loop。
#[derive(Debug)]
pub struct WorkerSupervisor {
    /// 重启预算。
    policy: WorkerRestartPolicy,
    /// 当前窗口开始时间。
    window_started_at: Option<Instant>,
    /// 当前窗口已经使用的重启次数。
    restart_count: u32,
}

impl WorkerSupervisor {
    /// 创建一个使用指定预算的 supervisor。
    pub fn new(policy: WorkerRestartPolicy) -> Result<Self, VisionError> {
        if policy.max_restarts == 0 || policy.window.is_zero() {
            return Err(VisionError::Protocol {
                message: "worker restart policy must allow a non-zero window".to_owned(),
            });
        }
        Ok(Self {
            policy,
            window_started_at: None,
            restart_count: 0,
        })
    }

    /// 记录一次 worker 故障并返回是否允许重启。
    pub fn on_failure(&mut self, now: Instant) -> RestartDecision {
        if self
            .window_started_at
            .is_none_or(|started_at| now.duration_since(started_at) >= self.policy.window)
        {
            self.window_started_at = Some(now);
            self.restart_count = 0;
        }
        if self.restart_count >= self.policy.max_restarts {
            RestartDecision::Fail
        } else {
            self.restart_count = self.restart_count.saturating_add(1);
            RestartDecision::Restart
        }
    }

    /// 返回当前窗口剩余的重启次数。
    pub fn remaining_restarts(&self) -> u32 {
        self.policy.max_restarts.saturating_sub(self.restart_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_budget_is_bounded_and_resets_after_window() {
        let policy = WorkerRestartPolicy {
            max_restarts: 2,
            window: Duration::from_secs(5),
        };
        let mut supervisor = WorkerSupervisor::new(policy).expect("policy is valid");
        let start = Instant::now();
        assert_eq!(supervisor.on_failure(start), RestartDecision::Restart);
        assert_eq!(supervisor.on_failure(start), RestartDecision::Restart);
        assert_eq!(supervisor.on_failure(start), RestartDecision::Fail);
        assert_eq!(
            supervisor.on_failure(start + Duration::from_secs(5)),
            RestartDecision::Restart
        );
    }
}
