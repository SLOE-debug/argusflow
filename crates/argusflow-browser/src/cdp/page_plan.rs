//! CDP 查询计划到页面解释器 DTO 的封闭序列化边界。

use argusflow_core::{ElementRole, PropertyPredicate};
use serde::Serialize;

use super::{CdpCandidateSource, CdpPlanExpr};

/// 页面函数接受的稳定查询 DTO，只包含执行所需字段。
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum PagePlan<'plan> {
    /// 角色和完整谓词集合。
    Match {
        /// 实际候选来源；页面解释器会拒绝尚未实现的来源。
        source: CdpCandidateSource,
        /// 目标语义角色。
        role: ElementRole,
        /// 页面内逐候选计算的完整谓词集合。
        predicates: Vec<&'plan PropertyPredicate>,
    },
    /// 后代关系。
    Descendant {
        /// 祖先计划。
        ancestor: Box<PagePlan<'plan>>,
        /// 后代目标计划。
        target: Box<PagePlan<'plan>>,
    },
    /// 直接子元素关系。
    Child {
        /// 父计划。
        parent: Box<PagePlan<'plan>>,
        /// 子目标计划。
        target: Box<PagePlan<'plan>>,
    },
    /// 当前 scope 结果补集。
    Not {
        /// 被排除的计划。
        query: Box<PagePlan<'plan>>,
    },
    /// 第一个结果。
    First {
        /// 内部计划。
        query: Box<PagePlan<'plan>>,
    },
    /// 一基索引结果。
    Nth {
        /// 内部计划。
        query: Box<PagePlan<'plan>>,
        /// 一基索引。
        index: usize,
    },
    /// 浏览器原生 CSS selector。
    Css {
        /// 完整 selector。
        selector: &'plan str,
    },
}

impl<'plan> From<&'plan CdpPlanExpr> for PagePlan<'plan> {
    fn from(expression: &'plan CdpPlanExpr) -> Self {
        match expression {
            CdpPlanExpr::Match(matcher) => Self::Match {
                source: matcher.source,
                role: matcher.role,
                predicates: matcher.predicates.iter().collect(),
            },
            CdpPlanExpr::Descendant { ancestor, target } => Self::Descendant {
                ancestor: Box::new(Self::from(ancestor.as_ref())),
                target: Box::new(Self::from(target.as_ref())),
            },
            CdpPlanExpr::Child { parent, target } => Self::Child {
                parent: Box::new(Self::from(parent.as_ref())),
                target: Box::new(Self::from(target.as_ref())),
            },
            CdpPlanExpr::Not(query) => Self::Not {
                query: Box::new(Self::from(query.as_ref())),
            },
            CdpPlanExpr::First(query) => Self::First {
                query: Box::new(Self::from(query.as_ref())),
            },
            CdpPlanExpr::Nth { query, index } => Self::Nth {
                query: Box::new(Self::from(query.as_ref())),
                index: *index,
            },
            CdpPlanExpr::Css { selector } => Self::Css { selector },
        }
    }
}
