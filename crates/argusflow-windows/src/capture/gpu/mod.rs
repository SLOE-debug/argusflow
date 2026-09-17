//! 有预算的 GPU 图像资源与完整帧缩放读回。
mod device;
mod error;
mod resources;
mod scale;
pub(in crate::capture) use device::Graphics;
pub(in crate::capture) use error::{failure, invalid};
pub(in crate::capture) use resources::{Buffer, Texture};
pub(in crate::capture) use scale::Scaler;
#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/gpu.rs"]
mod tests;
