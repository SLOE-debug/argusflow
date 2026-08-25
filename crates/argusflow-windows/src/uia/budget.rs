//! 单次 UIA 请求的 deadline、候选总量与关系根总量预算。

use std::time::{Duration, Instant};

use super::error::{UiaBudgetResource, UiaError};

/// 从进入 runtime channel 起生效的不可变 UIA 请求预算。
#[derive(Debug, Clone, Copy)]
pub(crate) struct UiaExecutionBudget {
    /// 包含排队时间的请求截止时刻。
    deadline: Instant,
    /// 单次请求允许从 provider 数组读取的最大候选总数。
    max_candidates: usize,
    /// 关系查询允许展开的最大父级/祖先根总数。
    max_relation_roots: usize,
}

impl UiaExecutionBudget {
    /// 从 runtime 的显式稳定配置创建一次请求预算。
    pub(crate) fn new(timeout: Duration, max_candidates: usize, max_relation_roots: usize) -> Self {
        debug_assert!(!timeout.is_zero());
        debug_assert!(max_candidates > 0);
        debug_assert!(max_relation_roots > 0);
        Self {
            deadline: Instant::now() + timeout,
            max_candidates,
            max_relation_roots,
        }
    }
}

/// worker 内部按整个请求累计资源消耗的可变计数器。
pub(crate) struct UiaBudgetTracker {
    /// 不随递归查询变化的请求限制。
    budget: UiaExecutionBudget,
    /// 所有 FindAllBuildCache 返回数组的累计候选数。
    candidates: usize,
    /// 所有 Child/Descendant 展开的累计关系根数。
    relation_roots: usize,
}

impl UiaBudgetTracker {
    /// 从不可变请求预算创建空计数器。
    pub(crate) const fn new(budget: UiaExecutionBudget) -> Self {
        Self {
            budget,
            candidates: 0,
            relation_roots: 0,
        }
    }

    /// 在每次可能阻塞的 provider 调用前后检查 ArgusFlow 截止时刻。
    pub(crate) fn check_deadline(&self) -> Result<(), UiaError> {
        if Instant::now() >= self.budget.deadline {
            Err(UiaError::ExecutionDeadlineExceeded)
        } else {
            Ok(())
        }
    }

    /// 累计 provider 返回的候选数并拒绝继续读取超预算数组。
    pub(crate) fn observe_candidates(&mut self, count: usize) -> Result<(), UiaError> {
        self.candidates = self.candidates.saturating_add(count);
        if self.candidates > self.budget.max_candidates {
            Err(UiaError::BudgetExceeded {
                resource: UiaBudgetResource::Candidates,
                limit: self.budget.max_candidates,
                observed: self.candidates,
            })
        } else {
            Ok(())
        }
    }

    /// 累计关系根展开数，防止宽泛祖先查询触发乘法级子树扫描。
    pub(crate) fn observe_relation_roots(&mut self, count: usize) -> Result<(), UiaError> {
        self.relation_roots = self.relation_roots.saturating_add(count);
        if self.relation_roots > self.budget.max_relation_roots {
            Err(UiaError::BudgetExceeded {
                resource: UiaBudgetResource::RelationRoots,
                limit: self.budget.max_relation_roots,
                observed: self.relation_roots,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{UiaBudgetTracker, UiaExecutionBudget};
    use crate::uia::error::{UiaBudgetResource, UiaError};

    /// 候选限制按完整请求累计，而不是只检查单个 FindAll 数组。
    #[test]
    fn candidate_budget_accumulates_across_queries() {
        let mut tracker =
            UiaBudgetTracker::new(UiaExecutionBudget::new(Duration::from_secs(1), 3, 2));

        tracker
            .observe_candidates(2)
            .expect("first candidate batch should fit");
        assert!(matches!(
            tracker.observe_candidates(2),
            Err(UiaError::BudgetExceeded {
                resource: UiaBudgetResource::Candidates,
                limit: 3,
                observed: 4,
            })
        ));
    }

    /// 关系根限制独立于候选限制，防止宽祖先结果反复扫描子树。
    #[test]
    fn relation_root_budget_is_enforced() {
        let mut tracker =
            UiaBudgetTracker::new(UiaExecutionBudget::new(Duration::from_secs(1), 10, 1));

        assert!(matches!(
            tracker.observe_relation_roots(2),
            Err(UiaError::BudgetExceeded {
                resource: UiaBudgetResource::RelationRoots,
                ..
            })
        ));
    }
}
