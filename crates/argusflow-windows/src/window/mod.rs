//! 窗口定位及防止 HWND 复用的身份租约。
mod identity;
mod locator;
mod stamp;
pub use identity::WindowIdentity;
pub use locator::{WindowInfo, WindowLocator};
