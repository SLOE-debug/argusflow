//! 结构、词法和表达式校验。
mod action;
mod model;
mod prepare;
mod scope;
pub use model::PreparedWorkflow;
pub(crate) use model::*;
pub use prepare::prepare;
