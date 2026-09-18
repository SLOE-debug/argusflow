//! 窗口定位及防止 HWND 复用的身份租约。
mod geometry;
mod identity;
mod locator;
mod ownership;
mod stamp;
mod surfaces;
pub use identity::WindowIdentity;
pub use locator::{WindowInfo, WindowLocator};
