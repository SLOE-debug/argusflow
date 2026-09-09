//! 带有界请求生命周期的 CDP 浏览器基础能力。

mod browser;
mod cdp;
mod error;
mod page;
mod process;

pub use argusflow_core::OperationOptions;
pub use browser::{Browser, BrowserConfig, LaunchOptions};
pub use cdp::ConnectionState;
pub use error::BrowserError;
pub use page::{Element, ElementSnapshot, Page, PageInfo};

#[cfg(test)]
#[path = "../../../tests/argusflow-browser/unit/mod.rs"]
mod tests;
