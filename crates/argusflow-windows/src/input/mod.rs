//! Windows 前台窗口输入注入后端。

mod backend;
mod keyboard;
mod mouse;
mod surface_transform;
mod visual_resolver;
mod window_activation;

pub use backend::SendInputBackend;
pub use mouse::{MouseInputError, WheelSteps, inject_scroll_wheel};
pub use visual_resolver::WindowsVisualTargetMaterializer;
