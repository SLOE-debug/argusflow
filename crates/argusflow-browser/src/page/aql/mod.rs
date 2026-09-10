//! DOM/AX 快照查询与显式 frame、Shadow 边界适配。
mod action;
mod boundary;
mod context;
mod geometry;
mod model;
mod properties;
mod query;
mod snapshot;
pub(crate) use boundary::FrameSession;
pub use model::BrowserMatch;
