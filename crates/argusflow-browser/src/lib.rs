//! 浏览器自动化后端、隔离 Chromium 资源与 Chrome DevTools Protocol runtime。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

/// Chrome DevTools Protocol 查询规划与执行能力。
pub mod cdp;

mod backend;
mod runtime;

pub use backend::CdpBackend;
pub use runtime::CdpRuntime;
