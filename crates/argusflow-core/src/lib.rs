//! 平台与运行时无关的操作约束、错误和输入参数。
mod action;
mod operation;

pub use action::{ClickCount, CssPoint, ImagePoint, Key, MouseButton, ScreenPoint, ScrollAxis};
pub use operation::{CancelOnDrop, Effect, Failure, FailureKind, Operation, OperationOptions};
