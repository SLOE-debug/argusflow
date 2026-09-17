//! 完整桌面帧与视频采集，GPU 资源由所属工作线程持有。
mod clock;
mod desktop;
mod dpi;
mod frames;
mod gpu;
mod topology;
pub(crate) mod video;
pub use frames::DxgiFrameSource;
