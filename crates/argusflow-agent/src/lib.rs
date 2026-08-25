//! 自动化动作的执行上下文、PreparedPlan 契约与运行时路由器。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod backend;
mod context;
mod plan;
mod router;

pub use backend::ActionBackend;
pub use context::{
    AccessibilityContext, BrowserSessionContext, ContextFitness, ExecutionContext,
    ExecutionContextProvider, ProcessContext, StaticExecutionContext, VisualCacheContext,
    WindowContext,
};
pub use plan::{
    PlanExplain, PlanRejection, PlanStepExplain, PlanStepKind, PlanningReport, PreparedCandidate,
    PreparedExecution, PreparedPlan, RuntimeAvailability,
};
pub use router::{ActionRouter, ROUTE_TIE_BREAK_ORDER};
