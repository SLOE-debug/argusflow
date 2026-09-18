//! 工作流任务与自动化能力之间的装配边界。
mod browser;
#[cfg(windows)]
mod desktop;
mod files;
mod query;
mod registration;
mod resources;
pub use query::{
    TargetConfig, TargetPlatform, TextEntryMode, TypeTargetConfig, WaitCondition, WaitTargetConfig,
};
pub use registration::register_automation;
#[cfg(windows)]
pub use resources::OcrServices;
pub use resources::QuerySourceProvider;
pub use resources::{AutomationHost, BrowserResource, PageResource, QuerySourceResource};
