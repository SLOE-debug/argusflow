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
mod component;
mod error;
mod execution;
mod node_registry;
mod resource;
mod trace;
mod validation;
mod value_runtime;

pub use application::UnavailableApplicationSessionProvider;
pub use browser::UnavailableBrowserSessionProvider;
pub use command::{CommandError, CommandExecutor};
pub use component::{
    ComponentExpansionError, ComponentSourceFrame, ComponentSourceMap, ExpandedWorkflow,
    MAX_COMPONENT_DEPTH, expand_components,
};
pub use component::{ComponentRegistry, ComponentRegistryError};
pub use error::RuntimeError;
pub use execution::{AccessSet, ResourceAccess, ResourceAccessKey, ResourceAccessMode};
pub use execution::{ActionDispatcher, UnavailableActionDispatcher};
pub use execution::{ExecutionEventSink, WorkflowEngine};
pub use execution::{NodeEvent, NodeExecution};
pub use execution::{NodeOutcome, RunContext};
pub use node_registry::{
    NodeCompileError, NodeCompiler, NodeFlow, NodeRegistryError, NodeTypeRegistry,
    NodeValidationContext, PreparedNode, ResourceInput, ValueInput, ValueTypeId,
};
pub use resource::{ResourceCleanup, ResourceEntry, ResourceTable};
pub use trace::{
    FileRunTraceStore, ResolvedInputField, ResolvedInputSource, ResolvedNodeInputs,
    RunArtifactKind, RunArtifactSummary, RunDetails, RunManifest, RunNodeOutputs, RunNodeTrace,
    RunStatus, RunTraceEvent, RunTraceLevel, RunTraceSession, RunTraceStore,
};
pub use validation::{
    ValidationIssue, ValidationIssueCode, ValidationReport, validate_workflow,
    validate_workflow_with_components, validate_workflow_with_registry,
};
