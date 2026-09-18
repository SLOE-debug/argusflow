//! 查询只读取由后端提供的有界快照。
mod capability;
mod geometry;
mod matcher;
mod query;
mod spatial;
mod tree;
pub use capability::Capabilities;
pub use geometry::{Geometry, Rect};
pub use query::{QueryProgress, evaluate, evaluate_step, preview};
pub use spatial::{SpatialCandidate, SpatialPreview};
pub use tree::{Node, NodeKind, QueryTree};
