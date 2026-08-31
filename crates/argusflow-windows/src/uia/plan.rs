use std::num::NonZeroUsize;

use argusflow_core::{ActionCapability, UiQuery};
use argusflow_query::{BackendQueryCapability, Diagnostic};

use super::native::{
    UiaNativePredicate, UiaPropertyProjection, UiaResidualPredicate, UiaRoleConstraint,
};

/// 已完成 UIA pushdown/residual 拆分的查询计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaQueryPlan {
    /// 规范化后仍保留层级和组合关系的计划树。
    pub expression: UiaPlanExpr,
    /// 由 UIA compiler 根据实际 pushdown/residual 计划推导的能力摘要。
    pub capability: BackendQueryCapability,
    /// 生成该计划的规范化 AQL AST，供 explain 和诊断使用。
    pub normalized: UiQuery,
    /// UIA compiler 产生的结构化计划诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// UIA 查询执行器需要保持的关系与选择语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaPlanExpr {
    /// 单次 UIA 候选查找及 residual filter。
    Match(UiaMatcherPlan),
    /// 在祖先的 Descendants scope 内查找目标。
    Descendant {
        /// 祖先查询计划。
        ancestor: Box<UiaPlanExpr>,
        /// 后代目标计划。
        target: Box<UiaPlanExpr>,
    },
    /// 在父元素的 Children scope 内查找目标。
    Child {
        /// 父查询计划。
        parent: Box<UiaPlanExpr>,
        /// 直接子目标计划。
        target: Box<UiaPlanExpr>,
    },
    /// 选择第一个结果。
    First(Box<UiaPlanExpr>),
    /// 选择从一开始计数的第 N 个结果。
    Nth {
        /// 内部查询计划。
        query: Box<UiaPlanExpr>,
        /// 一基索引。
        index: NonZeroUsize,
    },
}

/// 选择算子向 matcher 和关系遍历下传的结果数量边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiaResultLimit {
    /// 必须物化全部结果，供唯一性判断或关系左侧展开。
    All,
    /// 达到指定非零结果数后即可停止遍历。
    AtMost(NonZeroUsize),
}

impl UiaResultLimit {
    /// 创建 First 对应的单结果边界。
    pub(crate) const fn first() -> Self {
        Self::AtMost(NonZeroUsize::MIN)
    }

    /// 创建 Nth 对应的有界结果数。
    pub(crate) const fn at_most(count: NonZeroUsize) -> Self {
        Self::AtMost(count)
    }

    /// 返回具体上限；All 不限制结果数。
    pub(crate) const fn maximum(self) -> Option<usize> {
        match self {
            Self::All => None,
            Self::AtMost(count) => Some(count.get()),
        }
    }

    /// 判断已收集数量是否满足当前有界请求。
    pub(crate) const fn is_reached(self, result_count: usize) -> bool {
        match self {
            Self::All => false,
            Self::AtMost(count) => result_count >= count.get(),
        }
    }
}

/// 单个 UIA 元素 matcher 的原生条件、缓存与本地过滤边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaMatcherPlan {
    /// 已映射为 UIA ControlType/property condition 的角色。
    pub role: UiaRoleConstraint,
    /// 可编译为 PropertyCondition/AndCondition/NotCondition 的谓词。
    pub pushdown: Vec<UiaNativePredicate>,
    /// residual filter 必须通过 CacheRequest 一次性读取的属性。
    pub cache: Vec<UiaPropertyProjection>,
    /// UIA 无法原生完整表达、需要在 Rust 中计算的谓词。
    pub residual: Vec<UiaResidualPredicate>,
}

/// prepare 阶段冻结的 UIA 动作策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaActionPlan {
    /// 要求目标实例提供 InvokePattern。
    Invoke,
    /// 要求目标实例提供可写的 ValuePattern。
    SetValue {
        /// 要写入的完整文本。
        value: String,
    },
}

/// 查询目标角色对已冻结 UIA 动作策略的静态支持程度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiaActionSupport {
    /// 目标角色按 UIA 规范要求提供对应 pattern。
    Native,
    /// 角色可能提供多个 pattern，必须在唯一目标实例上复验所需 pattern。
    RequiresRuntimePatternCheck,
    /// 当前动作策略无法保持该角色的点击或写值语义。
    Unsupported,
}

/// prepare 阶段冻结的 UIA 查询、动作及两者联合能力证明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaPreparedPlan {
    /// 已编译且保持 AQL 关系与 fallback 语义的查询计划。
    pub query: UiaQueryPlan,
    /// 不需要在 execute 阶段重新解释的动作策略。
    pub action: UiaActionPlan,
    /// 最终目标角色对动作 pattern 的静态证明程度。
    pub action_support: UiaActionSupport,
    /// 查询支持与动作支持合并后的 Planner 能力摘要。
    pub capability: BackendQueryCapability,
}

/// UIA semantic candidates 经过动作适配与唯一性解析后的稳定失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetResolutionFailure {
    /// 查询没有找到任何语义候选。
    NotFound,
    /// 动作适配后仍存在多个候选。
    Ambiguous {
        /// 具备动作能力的候选数量。
        matches: usize,
    },
    /// 查询找到了候选，但没有候选支持当前动作。
    ActionUnsupported {
        /// suitability filter 之前的语义候选数量。
        semantic_matches: usize,
        /// 当前动作要求的跨后端能力。
        required: ActionCapability,
    },
}
