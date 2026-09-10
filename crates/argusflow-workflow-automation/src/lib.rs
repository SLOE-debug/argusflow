//! 工作流任务与自动化能力之间的装配边界。
mod browser;
#[cfg(windows)]
mod desktop;
mod query;
mod registration;
mod resources;
pub use registration::register_automation;
pub use resources::{AutomationHost, BrowserResource, PageResource, QuerySourceResource};
