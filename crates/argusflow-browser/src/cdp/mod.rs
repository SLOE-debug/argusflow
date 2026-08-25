//! AQL 到 Chromium DOM/Accessibility 查询计划的编译边界。

mod compiler;
mod explain;
mod plan;

pub use compiler::{CdpQueryCompileError, compile_cdp_query};
pub(crate) use explain::explain_cdp_plan;
pub use plan::{CdpCandidateSource, CdpMatcherPlan, CdpPlanExpr, CdpQueryPlan};
