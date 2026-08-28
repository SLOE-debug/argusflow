//! Windows 前台窗口输入注入后端。

mod backend;
mod keyboard;
mod mouse;
mod visual_resolver;

pub use backend::SendInputBackend;
pub use mouse::{MouseInputError, inject_scroll_wheel};
pub use visual_resolver::WindowsVisualTargetResolver;
