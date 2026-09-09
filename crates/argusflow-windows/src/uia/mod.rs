//! UIA 查询、Pattern 操作、租约及专用 MTA 执行线程。
mod action;
mod config;
mod element;
mod query;
mod runtime;
mod worker;
pub use action::{ScrollAmount, SelectionAction, UiaAction};
pub use config::UiaConfig;
pub use element::{ControlType, ElementHandle, ElementSnapshot, Predicate, Query, SearchScope};
pub use runtime::{UiaRuntime, UiaState};
