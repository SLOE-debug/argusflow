//! 有界全屏帧流，复用 DXGI / D3D11 基础设施，不编码视频也不计算区域差分。
mod output;
mod service;
mod store;
mod worker;
pub use service::DxgiFrameSource;
