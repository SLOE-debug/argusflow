//! 适配器资源类型，不在工作流值中传递平台句柄。
mod access;
mod browser;
#[cfg(windows)]
mod desktop;
mod host;
mod page_claim;
mod types;
pub(crate) use access::*;
pub use browser::*;
#[cfg(windows)]
pub(crate) use desktop::*;
pub use host::*;
pub(crate) use types::*;
