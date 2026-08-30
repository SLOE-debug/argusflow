use crate::{BackendPolicy, ScreenPoint, TargetScope, UiQuery, VisualQuery};

/// 一次执行中已经解析完成的视觉后置条件。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedVisualPostcondition {
    /// 要求动作前后视觉上下文连续，且目标文本实例数量增加。
    NewText {
        /// 已冻结的视觉查询。
        query: VisualQuery,
        /// 已冻结的上下文连续性查询。
        stable_context: Vec<VisualQuery>,
    },
    /// 要求动作后的新鲜画面中唯一存在目标文本。
    TextPresent {
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
