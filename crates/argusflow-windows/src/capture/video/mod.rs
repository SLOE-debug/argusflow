//! 独立原生视频原型：固定 GPU 预算，QPC 索引和硬件编码，不依赖录制分析。
mod capture_loop;
mod decode;
mod encoder;
mod graphics;
mod journal;
mod model;
mod pixels;
mod runtime;
mod sample;
mod session;
mod storage;
mod thumbnail;
mod timing;
pub use thumbnail::VideoThumbnail;
mod writer;
pub use decode::{DecodedVideoFrame, VideoDecoder, decode_video_frame};
pub use model::{VideoError, VideoOptions, VideoReport};
pub use session::{record_desktop_video, record_desktop_video_until};

#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/video.rs"]
mod tests;
