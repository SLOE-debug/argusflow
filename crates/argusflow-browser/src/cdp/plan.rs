use argusflow_core::{ElementRole, PropertyPredicate, UiQuery};
use argusflow_query::{BackendQueryCapability, Diagnostic};
use serde::Serialize;

/// 已选择 CDP 候选来源并冻结执行语义的查询计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpQueryPlan {
    /// DOM/AX 叶子计划及查询关系树。
    pub expression: CdpPlanExpr,
    /// 由 CDP compiler 根据真实执行路径推导的能力摘要。
    pub capability: BackendQueryCapability,
    /// 生成计划的规范化 AQL AST。
    pub normalized: UiQuery,
    /// CDP compiler 产生的结构化计划诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// CDP planner 选择的候选节点数据源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CdpCandidateSource {
    /// Chromium Accessibility tree；页面执行器实现前不得由 compiler 生成。
    AccessibilityTree,
    /// Chromium DOM tree。
    Dom,
}

/// CDP 执行器需要保持的查询结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdpPlanExpr {
    /// AX/DOM 候选查询与页面内谓词计算。
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

/// 单个语义 matcher 在 Chromium 中的真实执行输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpMatcherPlan {
    /// 执行器必须使用的候选节点来源，不得在 DTO 转换时丢弃。
    pub source: CdpCandidateSource,
    /// 页面解释器需要匹配的语义角色。
    pub role: ElementRole,
    /// 页面解释器逐候选计算的完整谓词集合。
    pub predicates: Vec<PropertyPredicate>,
}
