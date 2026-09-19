//! 从不可变录制事实生成可审核工作流；不启动应用或回放动作。
mod config;
mod error;
mod evidence;
mod inference;
mod storage;
mod transport;
pub use config::{AiConfig, ConfigView, SaveConfig};
pub use error::{AiError, Result};
pub use evidence::Evidence;
pub use inference::{
    Analysis, Binding, BindingKind, InferenceResult, Metrics, NodeEvidence, Outcome, Progress,
    ProgressStage, Unresolved, analyze,
};
pub use storage::ConfigStore;
pub use tokio_util::sync::CancellationToken;
