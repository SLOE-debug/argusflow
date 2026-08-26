//! ArgusFlow 工作流校验与执行运行时。
//!
//! 运行时把开放节点定义编译成强类型计划，按 RunWorld 隔离状态，并依据资源访问集合
//! 仲裁跨运行并发；具体自动化动作由注入的后端负责。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

mod application;
mod browser;
mod builtin_nodes;
mod command;
mod command_job;
mod dispatcher;
mod engine;
mod error;
mod node_execution;
mod node_registry;
mod resource_cleanup;
mod resource_table;
mod run_context;
mod run_inputs;
mod scheduler;
mod validation_graph;
mod validation_references;
mod validator;

pub use application::UnavailableApplicationSessionProvider;
pub use browser::UnavailableBrowserSessionProvider;
pub use command::{CommandError, CommandExecutor};
pub use dispatcher::{ActionDispatcher, UnavailableActionDispatcher};
pub use engine::{ExecutionEventSink, WorkflowEngine};
pub use error::RuntimeError;
pub use node_execution::{NodeEvent, NodeExecution};
pub use node_registry::{
    NodeCompileError, NodeCompiler, NodeFlow, NodeRegistryError, NodeTypeRegistry,
    NodeValidationContext, PreparedNode, ResourceInput, ValueInput, ValueTypeId,
};
pub use resource_table::{ResourceCleanup, ResourceEntry, ResourceTable};
pub use run_context::{NodeOutcome, RunContext};
pub use scheduler::{AccessSet, ResourceAccess, ResourceAccessKey, ResourceAccessMode};
pub use validator::{
    ValidationIssue, ValidationIssueCode, ValidationReport, validate_workflow,
    validate_workflow_with_registry,
};
