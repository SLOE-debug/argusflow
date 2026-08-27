use std::collections::BTreeMap;

use argusflow_core::{FlowComponentDefinition, FlowComponentId, FlowComponentVersion};

/// 以组件稳定 ID 和精确版本为键的只读运行时目录。
#[derive(Debug, Clone, Default)]
pub struct ComponentRegistry {
    /// BTreeMap 保证重复定义诊断和迭代顺序稳定。
    definitions: BTreeMap<(FlowComponentId, FlowComponentVersion), FlowComponentDefinition>,
}

impl ComponentRegistry {
    /// 创建空目录；不包含 Component 节点的工作流无需额外装配。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从一次运行随附的冻结组件定义建立目录。
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = FlowComponentDefinition>,
    ) -> Result<Self, ComponentRegistryError> {
        let mut registry = Self::new();
        for definition in definitions {
            registry.register(definition)?;
        }
        Ok(registry)
    }

    /// 注册一个精确版本；相同 ID/版本不允许被后来的内容覆盖。
    pub fn register(
        &mut self,
        definition: FlowComponentDefinition,
    ) -> Result<(), ComponentRegistryError> {
        if !is_exact_version(definition.version.as_str()) {
            return Err(ComponentRegistryError::InvalidVersion {
                version: definition.version.clone(),
            });
        }
        let key = (definition.id, definition.version.clone());
        if self.definitions.contains_key(&key) {
            return Err(ComponentRegistryError::DuplicateDefinition {
                component_id: definition.id,
                version: definition.version,
            });
        }
        self.definitions.insert(key, definition);
        Ok(())
    }

    /// 精确解析实例锁定的组件定义，不执行范围或 latest 匹配。
    pub fn resolve(
        &self,
        component_id: FlowComponentId,
        version: &FlowComponentVersion,
    ) -> Option<&FlowComponentDefinition> {
        self.definitions.get(&(component_id, version.clone()))
    }
}

/// 组件目录无法建立确定版本映射的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentRegistryError {
    /// 发布版本不是严格的 `major.minor.patch` 数字格式。
    InvalidVersion {
        /// 无效版本原值。
        version: FlowComponentVersion,
    },
    /// 相同稳定 ID 与精确版本出现多个定义。
    DuplicateDefinition {
        /// 冲突组件 ID。
        component_id: FlowComponentId,
        /// 冲突精确版本。
        version: FlowComponentVersion,
    },
}

impl std::fmt::Display for ComponentRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVersion { version } => write!(
                formatter,
                "component version '{}' must use exact major.minor.patch format",
                version.as_str(),
            ),
            Self::DuplicateDefinition {
                component_id,
                version,
            } => write!(
                formatter,
                "component '{}@{}' is defined more than once",
                component_id.as_uuid(),
                version.as_str(),
            ),
        }
    }
}

impl std::error::Error for ComponentRegistryError {}

/// P0 使用确定的三段数字版本，拒绝范围、标签和隐式 latest。
fn is_exact_version(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
