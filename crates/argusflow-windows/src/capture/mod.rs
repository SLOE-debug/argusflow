//! Windows 桌面和窗口画面捕获服务。

mod desktop_device;
mod desktop_output;
mod desktop_pixels;
#[cfg(test)]
mod desktop_pixels_tests;
mod desktop_readback;
mod device;
mod dpi;
mod error;
mod evidence;
mod evidence_desktop;
mod evidence_geometry;
mod host;
mod host_thread;
mod readback;
mod service;
mod wgc;
mod window_identity;
mod window_registry;
mod window_surface;

pub use evidence::WindowsEventCapture;
pub use host::WindowsCaptureHost;
pub use service::WindowsCaptureService;
pub use wgc::WindowsGraphicsCapture;
pub use window_registry::WindowsWindowRegistry;
