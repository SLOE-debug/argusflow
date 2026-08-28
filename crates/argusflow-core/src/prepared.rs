use crate::{AqlQuery, AutomationTarget, BackendPolicy, ScreenPoint, TargetScope, VisualQuery};

/// 一次执行中已经解析完成的视觉后置条件。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedVisualPostcondition {
    /// 要求动作后出现相对于 baseline 的新增文本。
    NewText {
        /// 已冻结的视觉查询。
        query: VisualQuery,
    },
}

/// 一次执行中已经解析完成的目标定位类别。
///
/// 该类型不实现序列化。它只存在于 Runtime 将值表达式冻结之后，避免把运行时状态
/// 写回工作流定义或让后端重新解析表达式。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedTargetLocator {
    /// 已解析的 AQL 查询。
    Query {
        /// 解析后的查询。
        query: AqlQuery,
    },
    /// 已冻结文字、范围和精确匹配规则的视觉查询。
    Visual {
        /// 解析后的视觉查询。
        query: VisualQuery,
    },
    /// 物理屏幕坐标。
    Coordinate {
        /// 目标屏幕点。
        point: ScreenPoint,
    },
    /// 当前键盘焦点。
    Focused,
}

/// 与一次执行绑定的目标准备结果。
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAutomationTarget {
    /// 已解析的资源作用域。
    scope: TargetScope,
    /// 已冻结、不可持久化的定位语义。
    locator: PreparedTargetLocator,
    /// 仍然适用于本次动作的后端候选策略。
    backend_policy: BackendPolicy,
}

impl PreparedAutomationTarget {
    /// 创建一次执行专用的目标准备结果。
    pub fn new(
        scope: TargetScope,
        locator: PreparedTargetLocator,
        backend_policy: BackendPolicy,
    ) -> Self {
        Self {
            scope,
            locator,
            backend_policy,
        }
    }

    /// 返回已解析作用域的只读引用。
    pub const fn scope(&self) -> &TargetScope {
        &self.scope
    }

    /// 返回已冻结定位器的只读引用。
    pub const fn locator(&self) -> &PreparedTargetLocator {
        &self.locator
    }

    /// 返回本次准备使用的后端策略。
    pub const fn backend_policy(&self) -> &BackendPolicy {
        &self.backend_policy
    }

    /// 将不需要表达式解析的持久化定位器转换为准备态定位器。
    pub fn from_persisted(target: &AutomationTarget) -> Option<Self> {
        let locator = match &target.locator {
            crate::TargetLocator::Query { query } => PreparedTargetLocator::Query {
                query: query.clone(),
            },
            crate::TargetLocator::Coordinate { point } => {
                PreparedTargetLocator::Coordinate { point: *point }
            }
            crate::TargetLocator::Focused => PreparedTargetLocator::Focused,
            crate::TargetLocator::Visual { .. } => return None,
        };
        Some(Self::new(
            target.scope.clone(),
            locator,
            target.backend_policy.clone(),
        ))
    }
}
