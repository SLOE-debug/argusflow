use argusflow_core::{ElementRole, PropertyPredicate, SelectorAttribute, UiQuery};
use argusflow_query::{BackendQueryCapability, Diagnostic};

/// 已选择 CDP 候选来源并完成 residual 拆分的查询计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpQueryPlan {
    /// DOM/AX 叶子计划及查询关系树。
    pub expression: CdpPlanExpr,
    /// 由 CDP compiler 根据真实候选源和 residual 计划推导的能力摘要。
    pub capability: BackendQueryCapability,
    /// 生成计划的规范化 AQL AST。
    pub normalized: UiQuery,
    /// CDP compiler 产生的结构化计划诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// CDP planner 选择的候选节点数据源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdpCandidateSource {
    /// Chromium Accessibility tree。
    AccessibilityTree,
    /// DOM tree 与原生 selector API。
    Dom,
}

/// CDP 执行器需要保持的查询结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdpPlanExpr {
    /// AX/DOM 候选查询与 residual filter。
    Match(CdpMatcherPlan),
    /// 在祖先后代中查找目标。
    Descendant {
        /// 祖先计划。
        ancestor: Box<CdpPlanExpr>,
        /// 后代目标计划。
        target: Box<CdpPlanExpr>,
    },
    /// 只查找父节点的直接子元素。
    Child {
        /// 父计划。
        parent: Box<CdpPlanExpr>,
        /// 子目标计划。
        target: Box<CdpPlanExpr>,
    },
    /// 按顺序尝试多个候选计划。
    Any(Vec<CdpPlanExpr>),
    /// 通过结果集合或树遍历排除内部计划。
    Not(Box<CdpPlanExpr>),
    /// 选择第一个结果。
    First(Box<CdpPlanExpr>),
    /// 选择从一开始计数的第 N 个结果。
    Nth {
        /// 内部查询计划。
        query: Box<CdpPlanExpr>,
        /// 一基索引。
        index: usize,
    },
    /// 直接交给 DOM.querySelectorAll 的原生 CSS fast path。
    Css {
        /// 不由 AQL 解释的 selector。
        selector: String,
    },
}

/// 单个语义 matcher 在 Chromium 中的 pushdown 与 residual 边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpMatcherPlan {
    /// 候选节点来源。
    pub source: CdpCandidateSource,
    /// 由 AX role 或 DOM tag/role 查询缩小候选范围的语义角色。
    pub role: ElementRole,
    /// 候选来源可原生表达的属性谓词。
    pub pushdown: Vec<PropertyPredicate>,
    /// residual filter 需要批量投影的属性。
    pub projected_attributes: Vec<SelectorAttribute>,
    /// 需要在 ArgusFlow 中计算的谓词。
    pub residual: Vec<PropertyPredicate>,
}
