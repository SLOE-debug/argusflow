//! Win32 应用进程、顶层窗口定位与激活服务。

mod application;
mod application_discovery;

pub use application::WindowsApplicationSessionProvider;
