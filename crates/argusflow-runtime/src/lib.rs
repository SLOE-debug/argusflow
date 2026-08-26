//! ArgusFlow 工作流校验与执行运行时。
//!
//! 运行时消费 `argusflow-core` 定义的契约，负责保证工作流结构有效、串行执行节点，
//! 并将生命周期事件发送给调用方；具体自动化动作由注入的后端负责。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod application;
mod command;
mod command_job;
mod dispatcher;
mod engine;
mod error;
mod node_executor;
mod run_context;
mod run_inputs;
mod validation_nodes;
mod validation_references;
mod validator;

pub use application::UnavailableApplicationSessionProvider;
pub use command::{CommandError, CommandExecutor};
pub use dispatcher::{ActionDispatcher, UnavailableActionDispatcher};
pub use engine::{ExecutionEventSink, WorkflowEngine};
pub use error::RuntimeError;
pub use run_context::{NodeOutcome, ResourceEntry, ResourceTable, RunContext};
pub use validator::{ValidationIssue, ValidationIssueCode, ValidationReport, validate_workflow};
