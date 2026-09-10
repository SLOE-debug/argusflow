//! 英文 AQL 到 UIA worker 的能力适配。
mod api;
mod focus;
mod geometry;
mod properties;
mod traversal;
pub use api::UiaMatch;
pub(super) use focus::focus;
pub(super) use geometry::click_point;
pub(super) use traversal::find;
