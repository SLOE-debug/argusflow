//! 可复用工作流组件的注册、展开和引用重写。

mod component_expander;
mod component_registry;
mod component_rewrite;

pub use component_expander::{
    ComponentExpansionError, ComponentSourceFrame, ComponentSourceMap, ExpandedWorkflow,
    MAX_COMPONENT_DEPTH, expand_components,
};
pub use component_registry::{ComponentRegistry, ComponentRegistryError};
