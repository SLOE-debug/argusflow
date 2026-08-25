use argusflow_core::{MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use argusflow_query::{
    BackendQueryCapability, Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend,
    QueryCost, SupportLevel, normalize_query,
};
use thiserror::Error;

use super::plan::{UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan};

/// UIA compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UiaQueryCompileError {
    /// 查询没有任何可由 UIA 保持语义的分支。
    #[error("AQL query has no branch that Windows UI Automation can execute")]
    UnsupportedQuery,
}

/// 单棵 UIA 表达式及由真实编译结果推导的摘要。
struct CompiledExpression {
    /// 可直接交给 UIA executor 的逻辑计划。
    expression: UiaPlanExpr,
    /// 原生、混合或模拟支持等级。
    support: SupportLevel,
    /// 实际计划的粗粒度成本。
    cost: QueryCost,
    /// 与 residual 或树遍历有关的结构化诊断。
    diagnostics: Vec<Diagnostic>,
}

/// 将查询编译为 UIA pushdown、CacheRequest 和 residual filter 计划。
pub fn compile_uia_query(query: &UiQuery) -> Result<UiaQueryPlan, UiaQueryCompileError> {
    let normalized = normalize_query(query);
    let compiled = compile_expression(&normalized.expression)?;
    Ok(UiaQueryPlan {
        expression: compiled.expression,
        capability: BackendQueryCapability {
            backend: QueryBackend::WindowsUia,
            level: compiled.support,
            estimated_cost: compiled.cost,
        },
        normalized,
        diagnostics: compiled.diagnostics,
    })
}

/// 递归保持 Query Algebra，并允许 `any` 只采用当前后端可执行的分支。
fn compile_expression(expression: &QueryExpr) -> Result<CompiledExpression, UiaQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => compile_matcher(matcher),
        QueryExpr::Descendant { ancestor, target } => {
            let ancestor = compile_expression(ancestor)?;
            let target = compile_expression(target)?;
            Ok(combine_binary(ancestor, target, |ancestor, target| {
                UiaPlanExpr::Descendant {
                    ancestor: Box::new(ancestor),
                    target: Box::new(target),
                }
            }))
        }
        QueryExpr::Child { parent, target } => {
            let parent = compile_expression(parent)?;
            let target = compile_expression(target)?;
            Ok(combine_binary(parent, target, |parent, target| {
                UiaPlanExpr::Child {
                    parent: Box::new(parent),
                    target: Box::new(target),
                }
            }))
        }
        QueryExpr::Any { queries } => compile_any(queries),
        QueryExpr::Not { query } => {
            let compiled = compile_expression(query)?;
            Ok(emulated(
                UiaPlanExpr::Not(Box::new(compiled.expression)),
                compiled.diagnostics,
            ))
        }
        QueryExpr::First { query } => {
            let compiled = compile_expression(query)?;
            Ok(CompiledExpression {
                expression: UiaPlanExpr::First(Box::new(compiled.expression)),
                support: compiled.support,
                cost: compiled.cost,
                diagnostics: compiled.diagnostics,
            })
        }
        QueryExpr::Nth { query, index } => {
            let compiled = compile_expression(query)?;
            Ok(CompiledExpression {
                expression: UiaPlanExpr::Nth {
                    query: Box::new(compiled.expression),
                    index: index.get(),
                },
                support: max_support(compiled.support, SupportLevel::Hybrid),
                cost: max_cost(compiled.cost, QueryCost::Medium),
                diagnostics: compiled.diagnostics,
            })
        }
        QueryExpr::Css { .. } => Err(UiaQueryCompileError::UnsupportedQuery),
    }
}

/// 编译 matcher 并从实际 residual 列表推导 Hybrid 能力。
fn compile_matcher(
    matcher: &argusflow_core::ElementMatcher,
) -> Result<CompiledExpression, UiaQueryCompileError> {
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

    let has_residual = !residual.is_empty();
    let diagnostics = has_residual
        .then(|| {
            Diagnostic::global(
                DiagnosticCode::ResidualFilter,
                DiagnosticSeverity::Information,
                Some(QueryBackend::WindowsUia),
            )
        })
        .into_iter()
        .collect();
    Ok(CompiledExpression {
        expression: UiaPlanExpr::Match(UiaMatcherPlan {
            role: matcher.role,
            pushdown,
            cache,
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
        diagnostics,
    })
}

/// 编译 `any` 的每个独立分支，并丢弃当前 backend 无法表达的替代分支。
fn compile_any(queries: &[QueryExpr]) -> Result<CompiledExpression, UiaQueryCompileError> {
    let branches = queries
        .iter()
        .filter_map(|query| compile_expression(query).ok())
        .collect::<Vec<_>>();
    if branches.is_empty() {
        return Err(UiaQueryCompileError::UnsupportedQuery);
    }
    if branches.len() == 1 {
        return Ok(branches
            .into_iter()
            .next()
            .expect("one compiled branch exists"));
    }

    let mut expressions = Vec::new();
    let mut diagnostics = Vec::new();
    for branch in branches {
        expressions.push(branch.expression);
        diagnostics.extend(branch.diagnostics);
    }
    diagnostics.push(Diagnostic::global(
        DiagnosticCode::ExpensiveTraversal,
        DiagnosticSeverity::Information,
        Some(QueryBackend::WindowsUia),
    ));
    Ok(CompiledExpression {
        expression: UiaPlanExpr::Any(expressions),
        support: SupportLevel::Emulated,
        cost: QueryCost::High,
        diagnostics,
    })
}

/// 合并关系表达式两侧的真实能力摘要。
fn combine_binary(
    left: CompiledExpression,
    right: CompiledExpression,
    build: impl FnOnce(UiaPlanExpr, UiaPlanExpr) -> UiaPlanExpr,
) -> CompiledExpression {
    let support = max_support(left.support, right.support);
    let cost = max_cost(left.cost, right.cost);
    let diagnostics = left
        .diagnostics
        .into_iter()
        .chain(right.diagnostics)
        .collect();
    CompiledExpression {
        expression: build(left.expression, right.expression),
        support,
        cost,
        diagnostics,
    }
}

/// 把需要结果集合计算的组合器标记为模拟执行。
fn emulated(expression: UiaPlanExpr, mut diagnostics: Vec<Diagnostic>) -> CompiledExpression {
    diagnostics.push(Diagnostic::global(
        DiagnosticCode::ExpensiveTraversal,
        DiagnosticSeverity::Information,
        Some(QueryBackend::WindowsUia),
    ));
    CompiledExpression {
        expression,
        support: SupportLevel::Emulated,
        cost: QueryCost::High,
        diagnostics,
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
