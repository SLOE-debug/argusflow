//! AQL 到 Chromium DOM/Accessibility 查询计划的编译边界。

mod compiler;
mod executor;
mod explain;
mod failure;
mod lifecycle;
mod page_script;
mod plan;
mod protocol;
mod session;

pub use compiler::{CdpQueryCompileError, compile_cdp_query};
pub(crate) use executor::execute_cdp_action;
pub(crate) use explain::explain_cdp_plan;
pub use plan::{CdpCandidateSource, CdpMatcherPlan, CdpPlanExpr, CdpQueryPlan};
pub(crate) use protocol::CdpConnection;
pub(crate) use session::{CdpPageSession, CdpSessionRegistry};
