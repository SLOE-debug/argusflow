//! UIA 查询、Pattern 操作、租约及专用 MTA 执行线程。
mod action;
mod aql;
mod config;
mod element;
mod query;
mod runtime;
mod text;
mod worker;
pub use action::{ScrollAmount, SelectionAction, UiaAction};
pub use aql::UiaMatch;
pub use config::UiaConfig;
pub use element::{ControlType, ElementHandle, ElementSnapshot, Predicate, Query, SearchScope};
pub use runtime::{UiaRuntime, UiaState};
pub use text::{UiaTextChange, UiaTextObservation, UiaTextRange, UiaTextSnapshot};
mod observation;
pub use observation::{UiaObservation, UiaObservedNode};
