//! 从时间锚点到稳定终点的视觉观察，不推断业务因果。
mod anchor;
mod model;
mod summary;
mod wait;
pub use model::{Anchor, Observation, ObservationStatus, ProcessSummary};
pub(crate) use summary::summarize;
