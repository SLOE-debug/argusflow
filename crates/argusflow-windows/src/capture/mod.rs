//! DXGI 可见桌面采样，所有 immediate context 调用属于适配器工作线程。
mod backend;
mod clock;
mod desktop;
mod dpi;
mod gpu;
mod lease;
mod output;
mod pixels;
mod queue;
mod requests;
mod topology;
mod worker;
pub use backend::DxgiBackend;
