//! 具体来源和最小能力契约在装配边界连接。
mod browser;
mod contract;
#[cfg(windows)]
mod input;
mod model;
#[cfg(windows)]
mod ocr;
#[cfg(windows)]
mod uia;
pub(crate) use browser::BrowserSource;
pub(crate) use contract::FocusedInput;
pub(crate) use contract::SourceBackend;
pub use model::{LocatedElement, QuerySource};
#[cfg(windows)]
pub(crate) use ocr::OcrSource;
#[cfg(windows)]
pub(crate) use uia::UiaSource;
