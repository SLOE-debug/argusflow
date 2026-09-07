//! Managed CDP reverse inspection 的几何、协议与对象生命周期模块。

mod geometry;
mod inspector;
mod lease;
mod page;

#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod protocol_tests;
