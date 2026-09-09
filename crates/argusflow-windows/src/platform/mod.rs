//! Win32 错误、COM apartment 与原生线程实例所有权。
mod com;
mod ownership;
pub(crate) use com::{Apartment, failure, hwnd, pattern_failure};
pub(crate) use ownership::Owner;
