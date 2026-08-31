use crate::{BackendPolicy, ScreenPoint, TargetScope, UiQuery};

/// Runtime 已解析参数并完成类型检查的一条 AQL 查询。
///
/// 该对象只存在于一次执行中。后端只读取冻结 AST，不得重新访问工作流值环境。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAqlQuery {
    /// 已完成参数替换的强类型查询。
    query: UiQuery,
    /// 用于诊断与追踪的原始 AQL 源码。
    source: String,
}

impl PreparedAqlQuery {
    /// 创建一次执行专用的冻结 AQL 查询。
    pub fn new(query: UiQuery, source: impl Into<String>) -> Self {
        Self {
            query,
            source: source.into(),
        }
    }

    /// 返回后端可直接编译的查询 AST。
    pub const fn query(&self) -> &UiQuery {
        &self.query
    }

    /// 返回持久化定义中的原始 AQL 源码。
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// 一次执行中已经解析完成的视觉后置条件。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedVisualPostcondition {
    /// 要求动作前后视觉上下文连续，且出现新的空间匹配实例。
    MatchAdded {
        /// 已冻结的目标 AQL 查询。
        query: PreparedAqlQuery,
        /// 已冻结的上下文连续性查询。
        stable_context: Vec<PreparedAqlQuery>,
    },
    /// 要求动作前后视觉上下文连续，且一个既有空间匹配实例已经消失。
    MatchRemoved {
        /// 已冻结的目标 AQL 查询。
        query: PreparedAqlQuery,
        /// 已冻结的上下文连续性查询。
        stable_context: Vec<PreparedAqlQuery>,
    },
    /// 要求动作后的新鲜画面中唯一存在目标匹配。
    MatchPresent {
        /// 已冻结的目标 AQL 查询。
        query: PreparedAqlQuery,
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
        /// 已解析、完成参数绑定且可被各后端直接编译的查询。
        query: UiQuery,
        /// 用于错误与查询追踪的原始 AQL 源码。
        source: String,
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
}
