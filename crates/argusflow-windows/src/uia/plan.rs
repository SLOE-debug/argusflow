use argusflow_core::{ElementRole, PropertyPredicate, SelectorAttribute, UiQuery};
use argusflow_query::{BackendQueryCapability, Diagnostic};

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
    /// 按顺序尝试多个计划分支。
    Any(Vec<UiaPlanExpr>),
    /// 通过 TreeWalker 或结果集合排除内部计划。
    Not(Box<UiaPlanExpr>),
    /// 选择第一个结果。
    First(Box<UiaPlanExpr>),
    /// 选择从一开始计数的第 N 个结果。
    Nth {
        /// 内部查询计划。
        query: Box<UiaPlanExpr>,
        /// 一基索引。
        index: usize,
    },
}

/// 单个 UIA 元素 matcher 的原生条件、缓存与本地过滤边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiaMatcherPlan {
    /// 映射为 UIA ControlType condition 的语义角色。
    pub role: ElementRole,
    /// 可编译为 PropertyCondition/AndCondition/NotCondition 的谓词。
    pub pushdown: Vec<PropertyPredicate>,
    /// residual filter 必须通过 CacheRequest 一次性读取的属性。
    pub cache: Vec<SelectorAttribute>,
    /// UIA 无法原生完整表达、需要在 Rust 中计算的谓词。
    pub residual: Vec<PropertyPredicate>,
}
