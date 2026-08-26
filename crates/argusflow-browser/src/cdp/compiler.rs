use argusflow_core::{ElementMatcher, MatchOperator, QueryExpr, SelectorAttribute, UiQuery};
use argusflow_query::{
    AlternativeBudgetExceeded, AlternativeExpansionBudget, BackendQueryCapability, BranchPath,
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend, QueryCost, SupportLevel,
    normalize_query,
};
use thiserror::Error;

use super::plan::{CdpCandidateSource, CdpMatcherPlan, CdpPlanExpr, CdpQueryPlan};

/// CDP compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CdpQueryCompileError {
    /// 查询没有任何可由 CDP 保持语义的分支。
    #[error("AQL query has no branch that Chrome DevTools Protocol can execute")]
    UnsupportedQuery,
    /// 查询替代方案组合超过 compiler 的稳定物化上限。
    #[error(transparent)]
    AlternativeLimitExceeded(#[from] AlternativeBudgetExceeded),
}

/// 单棵 CDP 表达式及由真实编译结果推导的摘要。
#[derive(Clone)]
struct CompiledExpression {
    /// 可直接交给 CDP executor 的逻辑计划。
    expression: CdpPlanExpr,
    /// 原生、混合或模拟支持等级。
    support: SupportLevel,
    /// 实际计划的粗粒度成本。
    cost: QueryCost,
    /// 当前替代方案在完整查询 fallback 树中的稳定路径。
    branch_path: BranchPath,
    /// 与 residual 或树遍历有关的结构化诊断。
    diagnostics: Vec<Diagnostic>,
}

/// 将查询编译为彼此独立的 CDP fallback 替代方案。
pub fn compile_cdp_query(query: &UiQuery) -> Result<Vec<CdpQueryPlan>, CdpQueryCompileError> {
    let normalized = normalize_query(query);
    let alternatives = compile_expression(
        &normalized.expression,
        AlternativeExpansionBudget::default(),
    )?;
    Ok(alternatives
        .into_iter()
        .map(|compiled| CdpQueryPlan {
            expression: compiled.expression,
            capability: BackendQueryCapability {
                backend: QueryBackend::BrowserCdp,
                level: compiled.support,
                estimated_cost: compiled.cost,
                branch_path: compiled.branch_path,
            },
            normalized: normalized.clone(),
            diagnostics: compiled.diagnostics,
        })
        .collect())
}

/// 递归展开 Query Algebra，确保每个结果只对应一个完整 fallback 路径。
fn compile_expression(
    expression: &QueryExpr,
    budget: AlternativeExpansionBudget,
) -> Result<Vec<CompiledExpression>, CdpQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => compile_matcher(matcher).map(|compiled| vec![compiled]),
        QueryExpr::Descendant { ancestor, target } => {
            let ancestor = compile_expression(ancestor, budget)?;
            let target = compile_expression(target, budget)?;
            emulated_binary(ancestor, target, budget, |ancestor, target| {
                CdpPlanExpr::Descendant {
                    ancestor: Box::new(ancestor),
                    target: Box::new(target),
                }
            })
        }
        QueryExpr::Child { parent, target } => {
            let parent = compile_expression(parent, budget)?;
            let target = compile_expression(target, budget)?;
            emulated_binary(parent, target, budget, |parent, target| {
                CdpPlanExpr::Child {
                    parent: Box::new(parent),
                    target: Box::new(target),
                }
            })
        }
        QueryExpr::Any { queries } => compile_any(queries, budget),
        QueryExpr::Not { query } => {
            let alternatives = compile_expression(query, budget)?;
            Ok(alternatives
                .into_iter()
                .map(|compiled| {
                    emulated(
                        CdpPlanExpr::Not(Box::new(compiled.expression)),
                        compiled.diagnostics,
                        compiled.branch_path,
                    )
                })
                .collect())
        }
        QueryExpr::First { query } => {
            let alternatives = compile_expression(query, budget)?;
            Ok(alternatives
                .into_iter()
                .map(|compiled| CompiledExpression {
                    expression: CdpPlanExpr::First(Box::new(compiled.expression)),
                    support: compiled.support,
                    cost: compiled.cost,
                    branch_path: compiled.branch_path,
                    diagnostics: compiled.diagnostics,
                })
                .collect())
        }
        QueryExpr::Nth { query, index } => {
            let alternatives = compile_expression(query, budget)?;
            Ok(alternatives
                .into_iter()
                .map(|compiled| CompiledExpression {
                    expression: CdpPlanExpr::Nth {
                        query: Box::new(compiled.expression),
                        index: index.get(),
                    },
                    support: max_support(compiled.support, SupportLevel::Hybrid),
                    cost: max_cost(compiled.cost, QueryCost::Medium),
                    branch_path: compiled.branch_path,
                    diagnostics: compiled.diagnostics,
                })
                .collect())
        }
        QueryExpr::Css { selector } => Ok(vec![CompiledExpression {
            expression: CdpPlanExpr::Css {
                selector: selector.clone(),
            },
            support: SupportLevel::Native,
            cost: QueryCost::Low,
            branch_path: BranchPath::root(),
            diagnostics: Vec::new(),
        }]),
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
        branch_path: BranchPath::root(),
        diagnostics,
    })
}

/// 展开 `any` 的每条可执行分支，并把原始索引前缀写入每个独立替代方案。
fn compile_any(
    queries: &[QueryExpr],
    budget: AlternativeExpansionBudget,
) -> Result<Vec<CompiledExpression>, CdpQueryCompileError> {
    let mut alternatives = Vec::new();
    for (branch_index, query) in queries.iter().enumerate() {
        let branch_alternatives = match compile_expression(query, budget) {
            Ok(branch_alternatives) => branch_alternatives,
            Err(CdpQueryCompileError::UnsupportedQuery) => continue,
            Err(error) => return Err(error),
        };
        budget.checked_sum(alternatives.len(), branch_alternatives.len())?;
        for mut alternative in branch_alternatives {
            alternative.branch_path.prepend(branch_index);
            alternatives.push(alternative);
        }
    }
    if alternatives.is_empty() {
        return Err(CdpQueryCompileError::UnsupportedQuery);
    }
    Ok(alternatives)
}

/// 对关系表达式两侧做笛卡尔积，并按 CDP 多次查询事实标记为模拟计划。
fn emulated_binary(
    left: Vec<CompiledExpression>,
    right: Vec<CompiledExpression>,
    budget: AlternativeExpansionBudget,
    build: impl Fn(CdpPlanExpr, CdpPlanExpr) -> CdpPlanExpr,
) -> Result<Vec<CompiledExpression>, CdpQueryCompileError> {
    let capacity = budget.checked_product(left.len(), right.len())?;
    let mut combined = Vec::with_capacity(capacity);
    for left_alternative in left {
        for right_alternative in &right {
            let mut branch_path = left_alternative.branch_path.clone();
            branch_path.append(&right_alternative.branch_path);
            let diagnostics = left_alternative
                .diagnostics
                .iter()
                .cloned()
                .chain(right_alternative.diagnostics.iter().cloned())
                .collect();
            combined.push(emulated(
                build(
                    left_alternative.expression.clone(),
                    right_alternative.expression.clone(),
                ),
                diagnostics,
                branch_path,
            ));
        }
    }
    Ok(combined)
}

/// 标记需要额外树遍历或结果集合计算的计划。
fn emulated(
    expression: CdpPlanExpr,
    mut diagnostics: Vec<Diagnostic>,
    branch_path: BranchPath,
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
        branch_path,
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
