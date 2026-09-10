//! 显式来源绑定、定位与一次性动作编排；不解释平台查询协议。
mod locator;
mod source;
pub use locator::Locator;
pub use source::{LocatedElement, QuerySource};
