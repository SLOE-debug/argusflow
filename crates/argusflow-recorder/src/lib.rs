//! 追加式桌面录制领域、归一化及可迁移数据包；不依赖平台 API。
mod model;
mod normalize;
mod policy;
mod protocol;
mod storage;
pub use model::*;
pub use normalize::{Normalizer, RecordingWriter};
pub use policy::*;
pub use protocol::*;
pub use storage::*;
