//! 类型化资源所有权与有界回收。
mod contract;
mod pool;
pub use contract::*;
pub(crate) use pool::*;
