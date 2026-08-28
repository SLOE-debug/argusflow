use argusflow_core::{AutomationAction, BackendKind, PreparedAutomationTarget};

use crate::{ExecutionContext, MaterializedTarget, PlanRejection, PreparedCandidate};

/// 后端只负责把原始动作编译成绑定执行计划的分支候选；执行阶段不再接收原始动作。
pub trait ActionBackend: Send + Sync {
    /// 返回候选后端类别，供用户约束在 prepare 之前过滤。
    fn kind(&self) -> BackendKind;

    /// 使用真实 compiler、运行上下文和 executor 状态准备每条可执行 fallback 路径。
    ///
    /// 每个返回项必须只对应一个 `BranchPath`；不得在单个候选中继续持有跨后端排序的
    /// 多条 `any(...)` 分支。
    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection>;

    /// 使用 Runtime 已冻结的目标准备候选；默认实现供非视觉后端沿用原有编译入口。
    fn prepare_with_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        _prepared_target: Option<&PreparedAutomationTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        self.prepare(action, context)
    }

    /// 使用 Planner 已完成的视觉物化结果准备候选；默认实现供非视觉后端沿用既有入口。
    fn prepare_with_materialized_target(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
        prepared_target: Option<&PreparedAutomationTarget>,
        _materialized_target: Option<&MaterializedTarget>,
    ) -> Result<Vec<PreparedCandidate>, PlanRejection> {
        self.prepare_with_target(action, context, prepared_target)
    }
}
