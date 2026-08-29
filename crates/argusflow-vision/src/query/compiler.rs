//! AQL 查询到 Vision SceneIndex 计划的确定性编译。

use argusflow_core::{
    DistanceMetric, ElementRole, MatchOperator, PredicateValue, QueryExpr, SelectorAttribute,
    SpatialDirection, UiQuery,
};
use argusflow_query::{
    BackendQueryCapability, BranchPath, QueryBackend, QueryCost, SupportLevel, normalize_query,
};
use thiserror::Error;

/// Vision 文本索引可直接执行的谓词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionTextPredicate {
    /// normalized text 完全相等。
    Exact(String),
    /// normalized text 包含指定片段。
    Contains(String),
}

/// 不再包含通用 AQL 语义解释的 Vision 查询 IR。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionPlanExpr {
    /// 文本索引读取。
    TextLookup(VisionTextPredicate),
    /// 按声明顺序选择第一个非空分支。
    Any(Vec<VisionPlanExpr>),
    /// 显式选择 reading order 第一项。
    First(Box<VisionPlanExpr>),
    /// 显式选择 reading order 第 N 项。
    Nth {
        /// 输入候选计划。
        query: Box<VisionPlanExpr>,
        /// 从一开始计数的索引。
        index: usize,
    },
    /// 以唯一 anchor 为中心执行方向过滤与距离 rank。
    Nearest {
        /// 唯一锚点计划。
        anchor: Box<VisionPlanExpr>,
        /// 目标候选计划。
        target: Box<VisionPlanExpr>,
        /// anchor-relative 方向。
        direction: SpatialDirection,
        /// 从一开始计数的距离 rank。
        index: usize,
        /// 分辨率无关距离度量。
        metric: DistanceMetric,
    },
}

/// Vision compiler 冻结的完整查询计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionQueryPlan {
    /// 可直接由 SceneIndex executor 运行的根计划。
    pub root: VisionPlanExpr,
    /// 是否要求完整 scene 才能安全给出全局否定。
    pub needs_complete_scene: bool,
    /// 来自真实 compiler 的能力摘要。
    pub capability: BackendQueryCapability,
    /// 面向 Explain 的稳定步骤摘要。
    pub summary: Vec<String>,
}

/// Vision 无法保持输入 AQL 语义。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VisionQueryCompileError {
    /// 查询使用了视觉事实无法证明的角色、属性或关系。
    #[error("AQL query is not supported by the Vision scene compiler")]
    Unsupported,
    /// prepare 阶段仍存在未冻结参数。
    #[error("AQL query contains an unresolved parameter '${name}'")]
    UnresolvedParameter {
        /// 未冻结参数名。
        name: String,
    },
}

/// 编译一条已解析且参数已冻结的 AQL 查询。
pub fn compile_vision_query(query: &UiQuery) -> Result<VisionQueryPlan, VisionQueryCompileError> {
    let normalized = normalize_query(query);
    let root = compile_expression(&normalized.expression)?;
    let summary = summarize(&root);
    Ok(VisionQueryPlan {
        root,
        needs_complete_scene: true,
        capability: BackendQueryCapability {
            backend: QueryBackend::Vision,
            level: SupportLevel::Native,
            estimated_cost: QueryCost::Medium,
            branch_path: BranchPath::root(),
        },
        summary,
    })
}

/// 递归降低受支持的 QueryExpr。
fn compile_expression(expression: &QueryExpr) -> Result<VisionPlanExpr, VisionQueryCompileError> {
    Ok(match expression {
        QueryExpr::Match { matcher } => {
            if matcher.role != ElementRole::Text || matcher.predicates.len() != 1 {
                return Err(VisionQueryCompileError::Unsupported);
            }
            let predicate = &matcher.predicates[0];
            if predicate.attribute != SelectorAttribute::Name {
                return Err(VisionQueryCompileError::Unsupported);
            }
            let text = match &predicate.value {
                PredicateValue::Text(text) => text.clone(),
                PredicateValue::Parameter(parameter) => {
                    return Err(VisionQueryCompileError::UnresolvedParameter {
                        name: parameter.name.clone(),
                    });
                }
                PredicateValue::Boolean(_) | PredicateValue::Regex(_) => {
                    return Err(VisionQueryCompileError::Unsupported);
                }
            };
            let predicate = match predicate.operator {
                MatchOperator::Equal => VisionTextPredicate::Exact(text),
                MatchOperator::Contains => VisionTextPredicate::Contains(text),
                MatchOperator::NotEqual
                | MatchOperator::StartsWith
                | MatchOperator::EndsWith
                | MatchOperator::Regex => return Err(VisionQueryCompileError::Unsupported),
            };
            VisionPlanExpr::TextLookup(predicate)
        }
        QueryExpr::Any { queries } => VisionPlanExpr::Any(
            queries
                .iter()
                .map(compile_expression)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        QueryExpr::First { query } => VisionPlanExpr::First(Box::new(compile_expression(query)?)),
        QueryExpr::Nth { query, index } => VisionPlanExpr::Nth {
            query: Box::new(compile_expression(query)?),
            index: index.get(),
        },
        QueryExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => VisionPlanExpr::Nearest {
            anchor: Box::new(compile_expression(anchor)?),
            target: Box::new(compile_expression(target)?),
            direction: *direction,
            index: index.get(),
            metric: *metric,
        },
        QueryExpr::Descendant { .. }
        | QueryExpr::Child { .. }
        | QueryExpr::Not { .. }
        | QueryExpr::Css { .. } => return Err(VisionQueryCompileError::Unsupported),
    })
}

/// 生成与冻结 IR 一致的 Explain 步骤。
fn summarize(expression: &VisionPlanExpr) -> Vec<String> {
    match expression {
        VisionPlanExpr::TextLookup(VisionTextPredicate::Exact(text)) => {
            vec![format!("exact TextIndex lookup: {text:?}")]
        }
        VisionPlanExpr::TextLookup(VisionTextPredicate::Contains(text)) => {
            vec![format!("contains text residual: {text:?}")]
        }
        VisionPlanExpr::Any(branches) => {
            vec![format!(
                "ordered any fallback with {} branches",
                branches.len()
            )]
        }
        VisionPlanExpr::First(_) => vec!["select reading-order result #1".to_owned()],
        VisionPlanExpr::Nth { index, .. } => {
            vec![format!("select reading-order result #{index}")]
        }
        VisionPlanExpr::Nearest {
            direction,
            index,
            metric,
            ..
        } => vec![format!(
            "filter direction={direction}, rank metric={metric}, select distance rank #{index}"
        )],
    }
}
