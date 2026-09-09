//! 原生线程上的有界按需读取与区域比较状态机。
mod compare;
mod model;
mod read;
pub(super) use model::{Request, Requests};
