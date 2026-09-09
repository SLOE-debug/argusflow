//! 有预算的 GPU 资源、精确差分与异步读回。
mod completion;
mod device;
mod difference;
mod error;
mod readback;
mod resources;
mod tiles;
pub(in crate::capture) use completion::Completion;
pub(in crate::capture) use device::Graphics;
pub(in crate::capture) use difference::{Difference, PendingDifference};
pub(in crate::capture) use error::{failure, invalid, recovery_reason};
pub(in crate::capture) use readback::PendingRead;
pub(in crate::capture) use resources::{Buffer, Texture};
pub(in crate::capture) use tiles::{TileBatch, TileMap, tiles_for_regions};

#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/gpu.rs"]
mod tests;
