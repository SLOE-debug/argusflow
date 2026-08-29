//! Windows 桌面和窗口画面捕获服务。

mod device;
mod dpi;
mod error;
mod host;
mod host_thread;
mod readback;
mod wgc;
mod window_identity;
mod window_registry;
mod window_surface;

pub use host::WindowsCaptureHost;
pub use wgc::WindowsGraphicsCapture;
pub use window_registry::WindowsWindowRegistry;
