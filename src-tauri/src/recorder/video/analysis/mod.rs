//! 有界像素变化分析与派生结果持久化。
mod cache;
mod model;
mod regions;
mod run;
mod selection;
pub(super) use run::analyze;
