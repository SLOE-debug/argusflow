//! 固定资源类型标识；值表达式不接收这些端口。
pub(crate) const BROWSER: &str = "automation.browser";
pub(crate) const PAGE: &str = "automation.page";
pub(crate) const SOURCE: &str = "automation.query_source";
#[cfg(windows)]
pub(crate) const APPLICATION: &str = "automation.application";
#[cfg(windows)]
pub(crate) const WINDOW: &str = "automation.window";
