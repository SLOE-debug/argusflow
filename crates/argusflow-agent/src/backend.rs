use argusflow_core::{AutomationAction, BackendKind};

use crate::{ExecutionContext, PlanRejection, PreparedCandidate};

/// 后端只负责把原始动作编译成绑定执行计划的候选；执行阶段不再接收原始动作。
pub trait ActionBackend: Send + Sync {
    /// 返回候选后端类别，供用户约束在 prepare 之前过滤。
    fn kind(&self) -> BackendKind;

    /// 使用真实 compiler、运行上下文和 executor 状态准备一个候选计划。
    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection>;
}
