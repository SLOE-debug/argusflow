//! CDP 请求关联、传输、会话状态和远端资源释放。
mod cleanup;
mod connection;
mod lifecycle;
mod protocol;
mod transport;
pub(crate) use cleanup::{Cleanup, UnknownResource};
pub(crate) use connection::Connection;
pub use lifecycle::ConnectionState;
pub(crate) use lifecycle::PageState;
