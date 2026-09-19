//! 多轮证据推断、证据覆盖审计和真实编译器校验。
mod model;
mod prompt;
mod service;
mod tools;
mod validation;
pub use model::{
    Analysis, Binding, BindingKind, InferenceResult, Metrics, NodeEvidence, Outcome, Progress,
    ProgressStage, Unresolved,
};
pub use service::analyze;
#[cfg(test)]
#[path = "../../../../tests/argusflow-ai/unit/mod.rs"]
mod tests;
