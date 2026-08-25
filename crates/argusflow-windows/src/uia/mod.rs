//! Windows UI Automation 后端。

mod action;
mod backend;
mod cache;
mod compiler;
mod condition;
mod error;
mod executor;
mod explain;
mod native;
mod plan;
mod property;
mod runtime;

pub use backend::UiaBackend;
pub use compiler::{UiaQueryCompileError, compile_uia_query};
pub use native::{
    UiaControlType, UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty,
    UiaPropertyProjection, UiaResidualMatcher, UiaResidualPredicate, UiaRoleConstraint,
};
pub use plan::{UiaActionPlan, UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan};
pub use runtime::{UiaRuntime, UiaRuntimeHealth, UiaRuntimeState};
