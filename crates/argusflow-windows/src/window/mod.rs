//! Win32 应用进程、顶层窗口定位与激活服务。

mod application;
mod application_discovery;
mod inspection;

pub use application::WindowsApplicationSessionProvider;
pub use inspection::WindowsWindowInspector;
