//! 本库创建的浏览器进程树和临时配置目录的所有权。
#[cfg(windows)]
mod job;
mod managed;
pub(crate) use managed::{Managed, reserve};
