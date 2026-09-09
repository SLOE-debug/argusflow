//! 操作截止时间、取消与失败上下文。
mod error;
mod ticket;
pub use error::{Effect, Failure, FailureKind};
pub use ticket::{CancelOnDrop, Operation, OperationOptions};
