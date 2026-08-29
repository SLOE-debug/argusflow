use argusflow_core::{
    ElementRole, MatchOperator, PredicateValue, PropertyPredicate, QueryExpr, SelectorAttribute,
    UiQuery, UiaAttribute,
};
use argusflow_query::{
    AlternativeBudgetExceeded, AlternativeExpansionBudget, BackendQueryCapability, BranchPath,
    Diagnostic, DiagnosticCode, DiagnosticSeverity, QueryBackend, QueryCost, SupportLevel,
    normalize_query,
};
use thiserror::Error;

use super::{
    native::{
        UiaControlType, UiaNativeComparison, UiaNativePredicate, UiaNativeValue, UiaProperty,
        UiaPropertyProjection, UiaResidualMatcher, UiaResidualPredicate, UiaResidualRegex,
        UiaRoleConstraint,
    },
    plan::{UiaMatcherPlan, UiaPlanExpr, UiaQueryPlan},
};

/// UIA compiler 无法保持查询语义时返回的结构化错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UiaQueryCompileError {
    /// 查询没有任何可由 UIA 保持语义的分支。
    #[error("AQL query has no branch that Windows UI Automation can execute")]
    UnsupportedQuery,
    /// parser 已接受的正则无法在 UIA 原生计划中预编译。
    #[error("AQL residual regular expression could not be compiled for Windows UI Automation")]
    InvalidResidualRegex,
    /// 查询替代方案组合超过 compiler 的稳定物化上限。
    #[error(transparent)]
    AlternativeLimitExceeded(#[from] AlternativeBudgetExceeded),
}

/// 单棵 UIA 表达式及由真实编译结果推导的摘要。
#[derive(Clone)]
struct CompiledExpression {
    /// 可直接交给 UIA executor 的逻辑计划。
    expression: UiaPlanExpr,
    /// 原生、混合或模拟支持等级。
    support: SupportLevel,
    /// 实际计划的粗粒度成本。
    cost: QueryCost,
    /// 当前替代方案在完整查询 fallback 树中的稳定路径。
    branch_path: BranchPath,
    /// 与 residual 或树遍历有关的结构化诊断。
    diagnostics: Vec<Diagnostic>,
}

/// 将查询编译为彼此独立的 UIA fallback 替代方案。
pub fn compile_uia_query(query: &UiQuery) -> Result<Vec<UiaQueryPlan>, UiaQueryCompileError> {
    let normalized = normalize_query(query);
    let alternatives = compile_expression(
        &normalized.expression,
        AlternativeExpansionBudget::default(),
    )?;
    Ok(alternatives
        .into_iter()
        .map(|compiled| UiaQueryPlan {
            expression: compiled.expression,
            capability: BackendQueryCapability {
                backend: QueryBackend::WindowsUia,
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
) -> Result<Vec<CompiledExpression>, UiaQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } => compile_matcher(matcher).map(|compiled| vec![compiled]),
        QueryExpr::Descendant { ancestor, target } => {
            let ancestor = compile_expression(ancestor, budget)?;
            let target = compile_expression(target, budget)?;
            combine_binary(ancestor, target, budget, |ancestor, target| {
                UiaPlanExpr::Descendant {
                    ancestor: Box::new(ancestor),
                    target: Box::new(target),
                }
            })
        }
        QueryExpr::Child { parent, target } => {
            let parent = compile_expression(parent, budget)?;
            let target = compile_expression(target, budget)?;
            combine_binary(parent, target, budget, |parent, target| {
                UiaPlanExpr::Child {
                    parent: Box::new(parent),
                    target: Box::new(target),
                }
            })
        }
        QueryExpr::Any { queries } => compile_any(queries, budget),
        QueryExpr::Not { .. } => Err(UiaQueryCompileError::UnsupportedQuery),
        QueryExpr::First { query } => {
            let alternatives = compile_expression(query, budget)?;
            Ok(alternatives
                .into_iter()
                .map(|compiled| CompiledExpression {
                    expression: UiaPlanExpr::First(Box::new(compiled.expression)),
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
                    expression: UiaPlanExpr::Nth {
                        query: Box::new(compiled.expression),
                        index: *index,
                    },
                    support: max_support(compiled.support, SupportLevel::Hybrid),
                    cost: max_cost(compiled.cost, QueryCost::Medium),
                    branch_path: compiled.branch_path,
                    diagnostics: compiled.diagnostics,
                })
                .collect())
        }
        QueryExpr::Css { .. } => Err(UiaQueryCompileError::UnsupportedQuery),
        QueryExpr::Nearest { .. } => Err(UiaQueryCompileError::UnsupportedQuery),
    }
}

/// 编译 matcher 并从实际 residual 列表推导 Hybrid 能力。
fn compile_matcher(
    matcher: &argusflow_core::ElementMatcher,
) -> Result<CompiledExpression, UiaQueryCompileError> {
    let role = compile_role(matcher.role)?;
    let mut pushdown = Vec::new();
    let mut residual = Vec::new();
    let mut cache = Vec::new();

    for predicate in &matcher.predicates {
        match compile_predicate(predicate)? {
            UiaPredicateCompilation::Pushdown(predicate) => pushdown.push(predicate),
            UiaPredicateCompilation::Residual {
                projection,
                predicate,
            } => {
                residual.push(predicate);
                if !cache.contains(&projection) {
                    cache.push(projection);
                }
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
            role,
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
        branch_path: BranchPath::root(),
        diagnostics,
    })
}

/// 单个 AQL 谓词编译后的原生或 residual 形态。
enum UiaPredicateCompilation {
    /// 可直接下推到 UIA condition。
    Pushdown(UiaNativePredicate),
    /// 需要精确缓存一个 UIA 属性并在本地比较。
    Residual {
        /// CacheRequest 使用的投影。
        projection: UiaPropertyProjection,
        /// executor 使用的本地谓词。
        predicate: UiaResidualPredicate,
    },
}

/// 把 portable role 降成确定的 UIA role constraint。
const fn compile_role(role: ElementRole) -> Result<UiaRoleConstraint, UiaQueryCompileError> {
    let control_type = match role {
        ElementRole::Window => UiaControlType::Window,
        ElementRole::Dialog => return Ok(UiaRoleConstraint::Dialog),
        ElementRole::Pane => UiaControlType::Pane,
        ElementRole::Button => UiaControlType::Button,
        ElementRole::TextBox => UiaControlType::Edit,
        ElementRole::CheckBox => UiaControlType::CheckBox,
        ElementRole::Radio => UiaControlType::RadioButton,
        ElementRole::ComboBox => UiaControlType::ComboBox,
        ElementRole::List => UiaControlType::List,
        ElementRole::ListItem => UiaControlType::ListItem,
        ElementRole::Tree => UiaControlType::Tree,
        ElementRole::TreeItem => UiaControlType::TreeItem,
        ElementRole::Tab => UiaControlType::Tab,
        ElementRole::TabItem => UiaControlType::TabItem,
        ElementRole::Menu => UiaControlType::Menu,
        ElementRole::MenuItem => UiaControlType::MenuItem,
        ElementRole::Link => UiaControlType::Hyperlink,
        ElementRole::Image => UiaControlType::Image,
        ElementRole::Table => UiaControlType::Table,
        ElementRole::Document => UiaControlType::Document,
        ElementRole::Text => UiaControlType::Text,
        ElementRole::Row | ElementRole::Cell => {
            return Err(UiaQueryCompileError::UnsupportedQuery);
        }
    };
    Ok(UiaRoleConstraint::ControlType(control_type))
}

/// 把 AQL 属性、值变换和比较方式全部冻结为 UIA 原生 IR。
fn compile_predicate(
    predicate: &PropertyPredicate,
) -> Result<UiaPredicateCompilation, UiaQueryCompileError> {
    let property = compile_property(predicate.attribute)?;
    match predicate.operator {
        MatchOperator::Equal | MatchOperator::NotEqual => {
            let value = compile_native_value(predicate.attribute, &predicate.value)?;
            let comparison = if matches!(predicate.operator, MatchOperator::Equal) {
                UiaNativeComparison::Equal(value)
            } else {
                UiaNativeComparison::NotEqual(value)
            };
            Ok(UiaPredicateCompilation::Pushdown(UiaNativePredicate {
                property,
                comparison,
            }))
        }
        MatchOperator::Contains | MatchOperator::StartsWith | MatchOperator::EndsWith => {
            if !is_string_property(property) {
                return Err(UiaQueryCompileError::UnsupportedQuery);
            }
            let PredicateValue::Text(text) = &predicate.value else {
                return Err(UiaQueryCompileError::UnsupportedQuery);
            };
            let matcher = match predicate.operator {
                MatchOperator::Contains => UiaResidualMatcher::Contains(text.clone()),
                MatchOperator::StartsWith => UiaResidualMatcher::StartsWith(text.clone()),
                MatchOperator::EndsWith => UiaResidualMatcher::EndsWith(text.clone()),
                _ => return Err(UiaQueryCompileError::UnsupportedQuery),
            };
            residual(property, matcher)
        }
        MatchOperator::Regex => {
            if !is_string_property(property) {
                return Err(UiaQueryCompileError::UnsupportedQuery);
            }
            let PredicateValue::Regex(regex) = &predicate.value else {
                return Err(UiaQueryCompileError::UnsupportedQuery);
            };
            residual(
                property,
                UiaResidualMatcher::Regex(
                    UiaResidualRegex::new(&regex.pattern, regex.case_insensitive)
                        .map_err(|_| UiaQueryCompileError::InvalidResidualRegex)?,
                ),
            )
        }
    }
}

/// 创建共享同一个强类型属性投影的 residual 计划。
fn residual(
    property: UiaProperty,
    matcher: UiaResidualMatcher,
) -> Result<UiaPredicateCompilation, UiaQueryCompileError> {
    let projection = UiaPropertyProjection::new(property);
    Ok(UiaPredicateCompilation::Residual {
        projection,
        predicate: UiaResidualPredicate {
            projection,
            matcher,
        },
    })
}

/// 映射 AQL 属性，不允许 executor 再次解释 portable 语义。
const fn compile_property(
    attribute: SelectorAttribute,
) -> Result<UiaProperty, UiaQueryCompileError> {
    Ok(match attribute {
        SelectorAttribute::Name => UiaProperty::Name,
        SelectorAttribute::Key | SelectorAttribute::Uia(UiaAttribute::AutomationId) => {
            UiaProperty::AutomationId
        }
        SelectorAttribute::Value => UiaProperty::Value,
        SelectorAttribute::Enabled => UiaProperty::IsEnabled,
        SelectorAttribute::Visible => UiaProperty::IsOffscreen,
        SelectorAttribute::Focused => UiaProperty::HasKeyboardFocus,
        SelectorAttribute::Checked => UiaProperty::ToggleState,
        SelectorAttribute::Selected => UiaProperty::IsSelected,
        SelectorAttribute::Uia(UiaAttribute::ClassName) => UiaProperty::ClassName,
        SelectorAttribute::Uia(UiaAttribute::AcceleratorKey) => UiaProperty::AcceleratorKey,
        SelectorAttribute::Uia(UiaAttribute::AccessKey) => UiaProperty::AccessKey,
        SelectorAttribute::Uia(UiaAttribute::FrameworkId) => UiaProperty::FrameworkId,
        SelectorAttribute::Dom(_) => return Err(UiaQueryCompileError::UnsupportedQuery),
    })
}

/// 映射原生 condition 右值，并在 compiler 内完成 visible/toggle 状态转换。
fn compile_native_value(
    attribute: SelectorAttribute,
    value: &PredicateValue,
) -> Result<UiaNativeValue, UiaQueryCompileError> {
    match (attribute, value) {
        (SelectorAttribute::Visible, PredicateValue::Boolean(value)) => {
            Ok(UiaNativeValue::Boolean(!*value))
        }
        (SelectorAttribute::Checked, PredicateValue::Boolean(value)) => {
            Ok(UiaNativeValue::Integer(if *value { 1 } else { 0 }))
        }
        (
            SelectorAttribute::Name
            | SelectorAttribute::Key
            | SelectorAttribute::Value
            | SelectorAttribute::Uia(_),
            PredicateValue::Text(value),
        ) => Ok(UiaNativeValue::Text(value.clone())),
        (
            SelectorAttribute::Enabled | SelectorAttribute::Focused | SelectorAttribute::Selected,
            PredicateValue::Boolean(value),
        ) => Ok(UiaNativeValue::Boolean(*value)),
        _ => Err(UiaQueryCompileError::UnsupportedQuery),
    }
}

/// 判断 UIA property 是否可以执行文本 residual 匹配。
const fn is_string_property(property: UiaProperty) -> bool {
    matches!(
        property,
        UiaProperty::Name
            | UiaProperty::AutomationId
            | UiaProperty::ClassName
            | UiaProperty::AcceleratorKey
            | UiaProperty::AccessKey
            | UiaProperty::FrameworkId
            | UiaProperty::Value
    )
}

/// 展开 `any` 的每条可执行分支，并把原始索引前缀写入每个独立替代方案。
fn compile_any(
    queries: &[QueryExpr],
    budget: AlternativeExpansionBudget,
) -> Result<Vec<CompiledExpression>, UiaQueryCompileError> {
    let mut alternatives = Vec::new();
    for (branch_index, query) in queries.iter().enumerate() {
        let branch_alternatives = match compile_expression(query, budget) {
            Ok(branch_alternatives) => branch_alternatives,
            Err(UiaQueryCompileError::UnsupportedQuery) => continue,
            Err(error) => return Err(error),
        };
        budget.checked_sum(alternatives.len(), branch_alternatives.len())?;
        for mut alternative in branch_alternatives {
            alternative.branch_path.prepend(branch_index);
            alternatives.push(alternative);
        }
    }
    if alternatives.is_empty() {
        return Err(UiaQueryCompileError::UnsupportedQuery);
    }
    Ok(alternatives)
}

/// 对关系表达式两侧做笛卡尔积，使每个结果冻结两侧所有 fallback 选择。
fn combine_binary(
    left: Vec<CompiledExpression>,
    right: Vec<CompiledExpression>,
    budget: AlternativeExpansionBudget,
    build: impl Fn(UiaPlanExpr, UiaPlanExpr) -> UiaPlanExpr,
) -> Result<Vec<CompiledExpression>, UiaQueryCompileError> {
    let capacity = budget.checked_product(left.len(), right.len())?;
    let mut combined = Vec::with_capacity(capacity);
    for left_alternative in left {
        for right_alternative in &right {
            combined.push(combine_binary_pair(
                left_alternative.clone(),
                right_alternative.clone(),
                &build,
            ));
        }
    }
    Ok(combined)
}

/// 合并一对已经确定分支选择的关系计划和能力摘要。
fn combine_binary_pair(
    left: CompiledExpression,
    right: CompiledExpression,
    build: &impl Fn(UiaPlanExpr, UiaPlanExpr) -> UiaPlanExpr,
) -> CompiledExpression {
    let support = max_support(left.support, right.support);
    let cost = max_cost(left.cost, right.cost);
    let mut branch_path = left.branch_path;
    branch_path.append(&right.branch_path);
    let diagnostics = left
        .diagnostics
        .into_iter()
        .chain(right.diagnostics)
        .collect();
    CompiledExpression {
        expression: build(left.expression, right.expression),
        support,
        cost,
        branch_path,
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
