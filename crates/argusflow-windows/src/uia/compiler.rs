use argusflow_core::{MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use argusflow_query::{QueryBackend, SupportLevel, analyze_query, normalize_query};
use thiserror::Error;

use super::plan::{UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan};

/// UIA compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UiaQueryCompileError {
    /// 查询包含 DOM/CSS 专用能力。
    #[error("AQL query contains properties that Windows UI Automation cannot execute")]
    UnsupportedQuery,
}

/// 将查询编译为 UIA pushdown、CacheRequest 和 residual filter 计划。
pub fn compile_uia_query(query: &UiQuery) -> Result<UiaQueryPlan, UiaQueryCompileError> {
    let normalized = normalize_query(query);
    let analysis = analyze_query(&normalized);
    let capability = analysis.capability(QueryBackend::WindowsUia);
    if capability.level == SupportLevel::Unsupported {
        return Err(UiaQueryCompileError::UnsupportedQuery);
    }

    let expression = compile_expression(&normalized.expression)?;
    Ok(UiaQueryPlan {
        expression,
        capability,
        normalized,
    })
}

/// 递归保留关系与组合器，并编译叶子 matcher。
fn compile_expression(expression: &QueryExpr) -> Result<UiaPlanExpr, UiaQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => {
            let mut pushdown = Vec::new();
            let mut residual = Vec::new();
            let mut cache = Vec::new();

            for predicate in &matcher.predicates {
                if matches!(predicate.attribute, SelectorAttribute::Dom(_)) {
                    return Err(UiaQueryCompileError::UnsupportedQuery);
                }
                if matches!(
                    predicate.operator,
                    MatchOperator::Equal | MatchOperator::NotEqual
                ) {
                    pushdown.push(predicate.clone());
                } else {
                    residual.push(predicate.clone());
                    if !cache.contains(&predicate.attribute) {
                        cache.push(predicate.attribute);
                    }
                }
            }
            cache.sort();

            Ok(UiaPlanExpr::Match(UiaMatcherPlan {
                role: matcher.role,
                pushdown,
                cache,
                residual,
            }))
        }
        QueryExpr::Descendant { ancestor, target } => Ok(UiaPlanExpr::Descendant {
            ancestor: Box::new(compile_expression(ancestor)?),
            target: Box::new(compile_expression(target)?),
        }),
        QueryExpr::Child { parent, target } => Ok(UiaPlanExpr::Child {
            parent: Box::new(compile_expression(parent)?),
            target: Box::new(compile_expression(target)?),
        }),
        QueryExpr::Any { queries } => Ok(UiaPlanExpr::Any(
            queries
                .iter()
                .map(compile_expression)
                .collect::<Result<_, _>>()?,
        )),
        QueryExpr::Not { query } => Ok(UiaPlanExpr::Not(Box::new(compile_expression(query)?))),
        QueryExpr::First { query } => Ok(UiaPlanExpr::First(Box::new(compile_expression(query)?))),
        QueryExpr::Nth { query, index } => Ok(UiaPlanExpr::Nth {
            query: Box::new(compile_expression(query)?),
            index: index.get(),
        }),
        QueryExpr::Css { .. } => Err(UiaQueryCompileError::UnsupportedQuery),
    }
}
