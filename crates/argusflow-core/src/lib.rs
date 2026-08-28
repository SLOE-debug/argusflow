//! ArgusFlow 的核心工作流契约与执行事件模型。
//!
//! 本 crate 只定义跨编辑器、运行时和自动化后端共享的数据结构，不负责实际执行。

mod action_options;
mod application;
mod automation;
mod browser;
mod command;
mod component;
mod condition;
mod data;
mod error;
mod execution;
mod input;
mod keyboard;
mod output;
mod prepared;
mod query;
mod resource;
mod value;
mod visual;
mod workflow;

pub use action_options::{
    ActionExecutionOptions, TargetWaitMode, TargetWaitPolicy, UiExecutionPolicy, UiPostcondition,
};
pub use application::{AcquirePolicy, ActivationPolicy, ApplicationSpec, CleanupPolicy};
pub use automation::{
    ActionOutcome, AutomationAction, AutomationExecutionScope, AutomationTarget, BackendKind,
    BackendPolicy, DiagnosticEvidenceReference, ExtractCardinality, FieldProjection,
    FieldProjectionSource, ScreenPoint, TargetLocator, TargetScope, UiOperation,
    WindowTitleMatcher,
};
pub use browser::{
    AcquireBrowserSpec, BrowserAcquireMode, BrowserCleanupPolicy, BrowserOperation, BrowserSpec,
};
pub use command::{
    CommandOperation, CommandRunner, EnvironmentBinding, WorkflowCapabilityId, WorkflowPermissions,
    required_command_capability,
};
pub use component::{
    ComponentInstance, ComponentValueOutput, FlowComponentDefinition, FlowComponentId,
    FlowComponentVersion,
};
pub use condition::{ConditionEvaluationError, ConditionOperator};
pub use data::DelimitedTextFormat;
pub use error::{ActionCapability, AutomationError};
pub use execution::{
    ExecutionComponentFrame, ExecutionEvent, ExecutionEventKind, ExecutionEventPayload, RunStarted,
};
pub use input::{RunInputs, WorkflowInputDefinition, WorkflowInputType};
pub use keyboard::{KeyChord, KeyboardKey, KeyboardModifier};
pub use output::{ActionOutputContract, ActionOutputKey, OutputContractError};
pub use prepared::{PreparedAutomationTarget, PreparedTargetLocator, PreparedVisualPostcondition};
pub use query::{
    AqlQuery, DomAttribute, ElementMatcher, ElementRole, MatchOperator, PredicateValue,
    PropertyPredicate, QueryExpr, QueryLanguageVersion, RegexLiteral, SelectorAttribute, UiQuery,
    UiaAttribute,
};
pub use resource::{
    AppSession, ApplicationError, ApplicationSessionProvider, BrowserError, BrowserSession,
    BrowserSessionProvider, CapabilityId, CapabilitySet, ProcessIdentity, ResourceId, ResourceRef,
    ResourceTypeId, WindowIdentity,
};
pub use value::{ValueExpr, ValueSource};
pub use visual::{NormalizedRect, VisualQuery, VisualQueryExpr};
pub use workflow::{
    ControlPortId, NodeEnvelope, NodeTypeId, Position, WorkflowDefinition, WorkflowEdge,
    WorkflowNode,
};
