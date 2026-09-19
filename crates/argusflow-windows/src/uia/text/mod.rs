//! TextPattern 事实采集，不聚焦、不调用 Select、不修改文档。
mod change;
mod identity;
mod model;
mod read;
pub use change::UiaTextChange;
pub(super) use identity::read as identity;
pub use model::{UiaTextObservation, UiaTextRange, UiaTextSnapshot};
pub(super) use read::observe;
