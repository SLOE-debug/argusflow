use argusflow_core::{ElementMatcher, MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use argusflow_query::{
    BackendQueryCapability, Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend,
    QueryCost, SupportLevel, normalize_query,
};
use thiserror::Error;

use super::plan::{CdpCandidateSource, CdpMatcherPlan, CdpPlanExpr, CdpQueryPlan};

/// CDP compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CdpQueryCompileError {
    /// 查询没有任何可由 CDP 保持语义的分支。
    #[error("AQL query has no branch that Chrome DevTools Protocol can execute")]
    UnsupportedQuery,
}

/// 单棵 CDP 表达式及由真实编译结果推导的摘要。
struct CompiledExpression {
    /// 可直接交给 CDP executor 的逻辑计划。
    expression: CdpPlanExpr,
    /// 原生、混合或模拟支持等级。
    support: SupportLevel,
    /// 实际计划的粗粒度成本。
    cost: QueryCost,
    /// 当前后端在最外层 `any` 中保留的最早原始分支索引。
    earliest_supported_branch_index: usize,
    /// 与 residual 或树遍历有关的结构化诊断。
    diagnostics: Vec<Diagnostic>,
}

/// 将查询编译为 DOM fast path 或 AX/DOM pushdown 加 residual 计划。
pub fn compile_cdp_query(query: &UiQuery) -> Result<CdpQueryPlan, CdpQueryCompileError> {
    let normalized = normalize_query(query);
    let compiled = compile_expression(&normalized.expression)?;
    Ok(CdpQueryPlan {
        expression: compiled.expression,
        capability: BackendQueryCapability {
            backend: QueryBackend::BrowserCdp,
            level: compiled.support,
            estimated_cost: compiled.cost,
            earliest_supported_branch_index: compiled.earliest_supported_branch_index,
        },
        normalized,
        diagnostics: compiled.diagnostics,
    })
}

/// 递归保持 Query Algebra，并允许 `any` 选择当前后端可执行的分支。
fn compile_expression(expression: &QueryExpr) -> Result<CompiledExpression, CdpQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => compile_matcher(matcher),
        QueryExpr::Descendant { ancestor, target } => {
            let ancestor = compile_expression(ancestor)?;
            let target = compile_expression(target)?;
            Ok(emulated_binary(ancestor, target, |ancestor, target| {
                CdpPlanExpr::Descendant {
                    ancestor: Box::new(ancestor),
                    target: Box::new(target),
                }
            }))
        }
        QueryExpr::Child { parent, target } => {
            let parent = compile_expression(parent)?;
            let target = compile_expression(target)?;
            Ok(emulated_binary(parent, target, |parent, target| {
                CdpPlanExpr::Child {
                    parent: Box::new(parent),
                    target: Box::new(target),
                }
            }))
        }
        QueryExpr::Any { queries } => compile_any(queries),
        QueryExpr::Not { query } => {
            let compiled = compile_expression(query)?;
            Ok(emulated(
                CdpPlanExpr::Not(Box::new(compiled.expression)),
                compiled.diagnostics,
                compiled.earliest_supported_branch_index,
            ))
        }
        QueryExpr::First { query } => {
            let compiled = compile_expression(query)?;
            Ok(CompiledExpression {
                expression: CdpPlanExpr::First(Box::new(compiled.expression)),
                support: compiled.support,
                cost: compiled.cost,
                earliest_supported_branch_index: compiled.earliest_supported_branch_index,
                diagnostics: compiled.diagnostics,
            })
        }
        QueryExpr::Nth { query, index } => {
            let compiled = compile_expression(query)?;
            Ok(CompiledExpression {
                expression: CdpPlanExpr::Nth {
                    query: Box::new(compiled.expression),
                    index: index.get(),
                },
                support: max_support(compiled.support, SupportLevel::Hybrid),
                cost: max_cost(compiled.cost, QueryCost::Medium),
                earliest_supported_branch_index: compiled.earliest_supported_branch_index,
                diagnostics: compiled.diagnostics,
            })
        }
        QueryExpr::Css { selector } => Ok(CompiledExpression {
            expression: CdpPlanExpr::Css {
                selector: selector.clone(),
            },
            support: SupportLevel::Native,
            cost: QueryCost::Low,
            earliest_supported_branch_index: 0,
            diagnostics: Vec::new(),
        }),
    }
}

/// 编译一个 AX/DOM matcher，并从 projected attributes 推导 Hybrid 能力。
fn compile_matcher(matcher: &ElementMatcher) -> Result<CompiledExpression, CdpQueryCompileError> {
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

    let has_residual = !residual.is_empty();
    let diagnostics = has_residual
        .then(|| {
            Diagnostic::global(
                DiagnosticCode::ResidualFilter,
                DiagnosticSeverity::Information,
                Some(QueryBackend::BrowserCdp),
            )
        })
        .into_iter()
        .collect();
    Ok(CompiledExpression {
        expression: CdpPlanExpr::Match(CdpMatcherPlan {
            source,
            role: matcher.role,
            pushdown,
            projected_attributes,
            residual,
        }),
        support: if has_residual {
            SupportLevel::Hybrid
        } else {
            SupportLevel::Native
        },
        cost: if has_residual {
            QueryCost::Medium
        } else {
            QueryCost::Low
        },
        earliest_supported_branch_index: 0,
        diagnostics,
    })
}

/// 编译 `any` 的独立分支，避免另一个 backend namespace 污染全部分支。
fn compile_any(queries: &[QueryExpr]) -> Result<CompiledExpression, CdpQueryCompileError> {
    let branches = queries
        .iter()
        .enumerate()
        .filter_map(|(index, query)| {
            compile_expression(query)
                .ok()
                .map(|compiled| (index, compiled))
        })
        .collect::<Vec<_>>();
    if branches.is_empty() {
        return Err(CdpQueryCompileError::UnsupportedQuery);
    }
    if branches.len() == 1 {
        let (index, mut branch) = branches
            .into_iter()
            .next()
            .expect("one compiled branch exists");
        branch.earliest_supported_branch_index = index;
        return Ok(branch);
    }

    let earliest_supported_branch_index = branches
        .first()
        .map(|(index, _)| *index)
        .expect("at least one compiled branch exists");
    let mut expressions = Vec::new();
    let mut diagnostics = Vec::new();
    for (_, branch) in branches {
        expressions.push(branch.expression);
        diagnostics.extend(branch.diagnostics);
    }
    diagnostics.push(Diagnostic::global(
        DiagnosticCode::ExpensiveTraversal,
        DiagnosticSeverity::Information,
        Some(QueryBackend::BrowserCdp),
    ));
    Ok(CompiledExpression {
        expression: CdpPlanExpr::Any(expressions),
        support: SupportLevel::Emulated,
        cost: QueryCost::High,
        earliest_supported_branch_index,
        diagnostics,
    })
}

/// 合并关系表达式并按 CDP 多次查询事实标记为模拟计划。
fn emulated_binary(
    left: CompiledExpression,
    right: CompiledExpression,
    build: impl FnOnce(CdpPlanExpr, CdpPlanExpr) -> CdpPlanExpr,
) -> CompiledExpression {
    let earliest_supported_branch_index = left
        .earliest_supported_branch_index
        .max(right.earliest_supported_branch_index);
    let diagnostics = left
        .diagnostics
        .into_iter()
        .chain(right.diagnostics)
        .collect();
    emulated(
        build(left.expression, right.expression),
        diagnostics,
        earliest_supported_branch_index,
    )
}

/// 标记需要额外树遍历或结果集合计算的计划。
fn emulated(
    expression: CdpPlanExpr,
    mut diagnostics: Vec<Diagnostic>,
    earliest_supported_branch_index: usize,
) -> CompiledExpression {
    diagnostics.push(Diagnostic::global(
        DiagnosticCode::ExpensiveTraversal,
        DiagnosticSeverity::Information,
        Some(QueryBackend::BrowserCdp),
    ));
    CompiledExpression {
        expression,
        support: SupportLevel::Emulated,
        cost: QueryCost::High,
        earliest_supported_branch_index,
        diagnostics,
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

/// 返回两个支持等级中实现成本更高的一项。
const fn max_support(left: SupportLevel, right: SupportLevel) -> SupportLevel {
    if left.rank() >= right.rank() {
        left
    } else {
        right
    }
}

/// 返回两个成本等级中更高的一项。
const fn max_cost(left: QueryCost, right: QueryCost) -> QueryCost {
    if left.rank() >= right.rank() {
        left
    } else {
        right
    }
}
