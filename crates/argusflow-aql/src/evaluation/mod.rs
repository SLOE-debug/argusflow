//! 查询只读取由后端提供的有界快照。
mod capability;
mod matcher;
mod query;
mod tree;
pub use capability::Capabilities;
pub use query::{QueryProgress, evaluate, evaluate_step};
pub use tree::{Node, NodeKind, QueryTree};
