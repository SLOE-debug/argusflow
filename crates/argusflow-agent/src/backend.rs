use argusflow_core::{
    AutomationAction, BackendKind, EntityObservation, ObservationRequest, ObservationUnknownReason,
    PreparedAutomationTarget,
};
use async_trait::async_trait;

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

/// 单一 UI 事实源执行 AQL v3 selector 叶节点的异步边界。
#[async_trait]
pub trait ObservationBackend: Send + Sync {
    /// 返回策略过滤和结果证据使用的稳定后端类别。
    fn kind(&self) -> BackendKind;

    /// 在同一后端状态快照中按表达式顺序求值全部 selector 叶节点。
    async fn observe(
        &self,
        request: &ObservationRequest,
        context: &ExecutionContext,
    ) -> Result<Vec<EntityObservation>, ObservationBackendError>;
}

/// 观察后端无法产生可交给统一求值层的事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationBackendError {
    /// 后端不支持查询语义，不应影响其它候选。
    Unsupported,
    /// 会话、窗口或运行服务暂不可用。
    Unavailable {
        /// 是否允许 Observe bounded 策略再次尝试整个路由。
        retryable: bool,
    },
    /// 后端已执行但无法给出完整、可信事实。
    Unknown {
        /// 稳定 Unknown 原因。
        reason: ObservationUnknownReason,
        /// 是否允许 Observe bounded 策略再次尝试整个路由。
        retryable: bool,
    },
}
