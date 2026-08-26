//! Windows UI Automation 后端。

mod action;
mod action_compiler;
mod backend;
mod budget;
mod cache;
mod compiler;
mod condition;
mod current_match;
mod element_identity;
mod error;
mod evidence;
mod executor;
mod explain;
mod native;
mod plan;
mod process_search;
mod property;
mod runtime;
mod runtime_worker;
mod selector_trace;
mod target_selection;

pub use action_compiler::{UiaActionCompileError, compile_uia_action};
pub use backend::UiaBackend;
pub use compiler::{UiaQueryCompileError, compile_uia_query};
pub use native::{
    UiaControlType, UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty,
    UiaPropertyProjection, UiaResidualMatcher, UiaResidualPredicate, UiaResidualRegex,
    UiaRoleConstraint,
};
pub use plan::{
    TargetResolutionFailure, TargetWaitPolicy, UiaActionPlan, UiaActionSupport, UiaMatcherPlan,
    UiaPlanExpr, UiaPreparedPlan, UiaQueryPlan,
};
pub use runtime::{UiaRuntime, UiaRuntimeHealth, UiaRuntimeState};
