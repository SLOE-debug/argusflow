//! 工作流结构、图形和数据引用校验。

mod validation_graph;
mod validation_references;
mod validation_scopes;
mod validation_workflow;
pub(crate) mod validator;

pub use validator::{
    ValidationIssue, ValidationIssueCode, ValidationReport, validate_workflow,
    validate_workflow_with_components, validate_workflow_with_registry,
};
