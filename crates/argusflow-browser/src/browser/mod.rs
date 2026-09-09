//! 浏览器连接、启动选项和页面装配。
mod client;
mod config;
mod endpoint;
pub use client::Browser;
pub use config::{BrowserConfig, LaunchOptions};
pub(crate) use endpoint::validate_url;
