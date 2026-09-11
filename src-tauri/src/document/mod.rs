//! 编辑文档契约、无损传输与原子持久化。
pub(crate) mod clipboard;
pub(crate) mod compilation;
mod model;
pub(crate) mod storage;
mod structure;
pub(crate) mod wire;
pub use model::*;
pub use storage::Workspace;
