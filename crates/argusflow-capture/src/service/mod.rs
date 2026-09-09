//! 共享服务的装配与稳定导出。
mod lifecycle;
mod model;
mod state;
mod subscription;
mod validation;
pub use lifecycle::CaptureService;
pub use model::{ChangeKind, ChangeRecord};
pub(crate) use state::State;
pub use subscription::{ChangeBatch, ChangeSubscription};
