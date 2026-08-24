use argusflow_core::{MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use argusflow_query::{QueryBackend, SupportLevel, analyze_query, normalize_query};
use thiserror::Error;

use super::plan::{CdpCandidateSource, CdpMatcherPlan, CdpPlanExpr, CdpQueryPlan};

/// CDP compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CdpQueryCompileError {
    /// 查询包含 UIA 专用属性。
    #[error("AQL query contains properties that Chrome DevTools Protocol cannot execute")]
    UnsupportedQuery,
}

/// 将查询编译为 DOM fast path 或 AX/DOM pushdown 加 residual 计划。
pub fn compile_cdp_query(query: &UiQuery) -> Result<CdpQueryPlan, CdpQueryCompileError> {
    let normalized = normalize_query(query);
    let analysis = analyze_query(&normalized);
    let capability = analysis.capability(QueryBackend::BrowserCdp);
    if capability.level == SupportLevel::Unsupported {
        return Err(CdpQueryCompileError::UnsupportedQuery);
    }

    let expression = compile_expression(&normalized.expression)?;
    Ok(CdpQueryPlan {
        expression,
        capability,
        normalized,
    })
}

/// 递归保留关系与组合器并编译叶子查询。
fn compile_expression(expression: &QueryExpr) -> Result<CdpPlanExpr, CdpQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => {
            if matcher
                .predicates
                .iter()
                .any(|predicate| matches!(predicate.attribute, SelectorAttribute::Uia(_)))
            {
                return Err(CdpQueryCompileError::UnsupportedQuery);
            }

            let source = if matcher
                .predicates
                .iter()
                .any(|predicate| matches!(predicate.attribute, SelectorAttribute::Dom(_)))
            {
                CdpCandidateSource::Dom
            } else {
                CdpCandidateSource::AccessibilityTree
            };
            let mut pushdown = Vec::new();
            let mut residual = Vec::new();
            let mut projected_attributes = Vec::new();
            for predicate in &matcher.predicates {
                if can_push_down(source, predicate.attribute, predicate.operator) {
                    pushdown.push(predicate.clone());
                } else {
                    residual.push(predicate.clone());
                    if !projected_attributes.contains(&predicate.attribute) {
                        projected_attributes.push(predicate.attribute);
                    }
                }
            }
            projected_attributes.sort();

            Ok(CdpPlanExpr::Match(CdpMatcherPlan {
                source,
                role: matcher.role,
                pushdown,
                projected_attributes,
                residual,
            }))
        }
        QueryExpr::Descendant { ancestor, target } => Ok(CdpPlanExpr::Descendant {
            ancestor: Box::new(compile_expression(ancestor)?),
            target: Box::new(compile_expression(target)?),
        }),
        QueryExpr::Child { parent, target } => Ok(CdpPlanExpr::Child {
            parent: Box::new(compile_expression(parent)?),
            target: Box::new(compile_expression(target)?),
        }),
        QueryExpr::Any { queries } => Ok(CdpPlanExpr::Any(
            queries
                .iter()
                .map(compile_expression)
                .collect::<Result<_, _>>()?,
        )),
        QueryExpr::Not { query } => Ok(CdpPlanExpr::Not(Box::new(compile_expression(query)?))),
        QueryExpr::First { query } => Ok(CdpPlanExpr::First(Box::new(compile_expression(query)?))),
        QueryExpr::Nth { query, index } => Ok(CdpPlanExpr::Nth {
            query: Box::new(compile_expression(query)?),
            index: index.get(),
        }),
        QueryExpr::Css { selector } => Ok(CdpPlanExpr::Css {
            selector: selector.clone(),
        }),
    }
}

/// 判断属性谓词能否由所选 CDP 数据源完整下推。
const fn can_push_down(
    source: CdpCandidateSource,
    attribute: SelectorAttribute,
    operator: MatchOperator,
) -> bool {
    if matches!(operator, MatchOperator::Regex) {
        return false;
    }

    match source {
        CdpCandidateSource::AccessibilityTree => {
            matches!(attribute, SelectorAttribute::Name) && matches!(operator, MatchOperator::Equal)
        }
        CdpCandidateSource::Dom => matches!(
            attribute,
            SelectorAttribute::Dom(_) | SelectorAttribute::Key
        ),
    }
}
