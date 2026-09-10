//! 具体来源和最小能力契约在装配边界连接。
mod browser;
mod contract;
#[cfg(windows)]
mod desktop;
mod model;
pub(crate) use browser::BrowserSource;
pub(crate) use contract::SourceBackend;
#[cfg(windows)]
pub(crate) use desktop::{OcrSource, UiaSource};
pub use model::{LocatedElement, QuerySource};
