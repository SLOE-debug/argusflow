//! ArgusFlow 的核心工作流契约与执行事件模型。
//!
//! 本 crate 只定义跨编辑器、运行时和自动化后端共享的数据结构，不负责实际执行。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod automation;
mod condition;
mod error;
mod execution;
mod workflow;

pub use automation::{ActionOutcome, AutomationAction, BackendKind, Selector};
pub use condition::{ConditionEvaluationError, ConditionOperator, ConditionPredicate};
pub use error::AutomationError;
pub use execution::{ExecutionEvent, ExecutionEventKind, RunStarted};
pub use workflow::{
    ConditionBranch, Position, WorkflowDefinition, WorkflowEdge, WorkflowNode, WorkflowNodeKind,
};
