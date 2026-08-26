use thiserror::Error;

/// Backend compiler 单次允许物化的最大查询替代方案数。
pub const MAX_COMPILED_ALTERNATIVES: usize = 4_096;

/// 查询替代方案展开的共享硬预算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternativeExpansionBudget {
    /// 任一中间表达式允许持有的最大替代方案数。
    limit: usize,
}

impl AlternativeExpansionBudget {
    /// 返回供诊断和测试观察的稳定上限。
    pub const fn limit(self) -> usize {
        self.limit
    }

    /// 在 `any(...)` 合并前验证加法不会溢出或超过硬上限。
    pub fn checked_sum(
        self,
        current: usize,
        additional: usize,
    ) -> Result<usize, AlternativeBudgetExceeded> {
        current
            .checked_add(additional)
            .filter(|total| *total <= self.limit)
            .ok_or(AlternativeBudgetExceeded { limit: self.limit })
    }

    /// 在关系表达式建立笛卡尔积前验证乘法不会溢出或超过硬上限。
    pub fn checked_product(
        self,
        left: usize,
        right: usize,
    ) -> Result<usize, AlternativeBudgetExceeded> {
        left.checked_mul(right)
            .filter(|total| *total <= self.limit)
            .ok_or(AlternativeBudgetExceeded { limit: self.limit })
    }
}

impl Default for AlternativeExpansionBudget {
    fn default() -> Self {
        Self {
            limit: MAX_COMPILED_ALTERNATIVES,
        }
    }
}

/// 查询替代方案数量超过 compiler 允许物化的稳定上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("query expansion exceeds the hard limit of {limit} alternatives")]
pub struct AlternativeBudgetExceeded {
    /// 本次编译采用的替代方案硬上限。
    limit: usize,
}

impl AlternativeBudgetExceeded {
    /// 返回触发拒绝的稳定上限。
    pub const fn limit(self) -> usize {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::{AlternativeExpansionBudget, MAX_COMPILED_ALTERNATIVES};

    /// 加法与乘法都必须在分配前拒绝溢出和超限结果。
    #[test]
    fn expansion_budget_rejects_oversized_or_overflowing_results() {
        let budget = AlternativeExpansionBudget::default();

        assert_eq!(budget.checked_sum(2_000, 2_000), Ok(4_000));
        assert!(budget.checked_sum(4_000, 100).is_err());
        assert_eq!(budget.checked_product(64, 64), Ok(4_096));
        assert!(budget.checked_product(65, 65).is_err());
        assert!(budget.checked_product(usize::MAX, 2).is_err());
        assert_eq!(budget.limit(), MAX_COMPILED_ALTERNATIVES);
    }
}
