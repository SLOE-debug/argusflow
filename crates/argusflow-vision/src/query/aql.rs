//! AQL 文本查询到 OCR Scene 的一次性编译与无分配热路径求值。

use std::{collections::BTreeMap, fmt, time::Instant};

use argusflow_core::{
    AutomationError, DistanceMetric, ElementRole, MatchOperator, PredicateValue, QueryExpr,
    SelectorAttribute, SpatialDirection, UiQuery,
};
use regex::{Regex, RegexBuilder};
use serde::Serialize;

use crate::{AppNodeRef, AppScene, PhysicalRect, normalize_text};

/// Vision 后端可直接执行的冻结查询计划。
#[derive(Debug)]
pub struct VisionQueryPlan {
    /// 已编译且不再含参数的表达式。
    expression: VisionExpr,
}

/// 单次求值的性能计数器，供 Explain 和运行追踪使用。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct VisionQueryMetrics {
    /// 求值总耗时，单位微秒。
    pub elapsed_us: u64,
    /// 命中精确全文倒排索引的窗口次数。
    pub exact_index_hits: usize,
    /// 执行 residual predicate 的节点数。
    pub scanned_nodes: usize,
    /// 进入空间方向与距离计算的候选数。
    pub spatial_candidates: usize,
}

/// 查询求值返回的借用节点和性能事实。
#[derive(Debug)]
pub struct VisionQueryResult<'scene> {
    /// 按窗口 Z-Order、节点 y/x 排列的候选。
    pub matches: Vec<AppNodeRef<'scene>>,
    /// 本次求值产生的热路径计数器。
    pub metrics: VisionQueryMetrics,
}

/// AQL 语义超出 OCR 文本 Scene 能力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionQueryCompileError {
    /// 面向 Explain 的稳定原因。
    message: String,
}

impl fmt::Display for VisionQueryCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for VisionQueryCompileError {}

/// 一次性验证角色/属性并预编译正则表达式。
pub fn compile_vision_query(query: &UiQuery) -> Result<VisionQueryPlan, VisionQueryCompileError> {
    Ok(VisionQueryPlan {
        expression: compile_expression(&query.expression)?,
    })
}

/// 在完整进程 Scene 上执行冻结计划。
pub fn evaluate_vision_query<'scene>(
    scene: &'scene AppScene,
    plan: &VisionQueryPlan,
    query_source: &str,
) -> Result<VisionQueryResult<'scene>, AutomationError> {
    let started = Instant::now();
    let mut metrics = VisionQueryMetrics::default();
    let window_scope = (0..scene.windows.len()).collect::<Vec<_>>();
    let matches = evaluate_expression(
        scene,
        &plan.expression,
        &window_scope,
        query_source,
        &mut metrics,
    )?;
    metrics.elapsed_us = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    Ok(VisionQueryResult { matches, metrics })
}

/// 对要求唯一目标的动作执行严格 0/1/N 判定。
pub fn require_unique<'scene>(
    result: &VisionQueryResult<'scene>,
    query_source: &str,
) -> Result<(AppNodeRef<'scene>, VisionQueryMetrics), AutomationError> {
    match result.matches.as_slice() {
        [] => Err(AutomationError::TargetNotFound {
            query: query_source.to_owned(),
            details: "complete OCR scene contains no matching text node".to_owned(),
        }),
        [candidate] => Ok((*candidate, result.metrics)),
        candidates => Err(AutomationError::AmbiguousTarget {
            query: query_source.to_owned(),
            matches: candidates.len(),
            details: "AQL matched multiple OCR text nodes; use first, nth, or nearest".to_owned(),
        }),
    }
}

#[derive(Debug)]
enum VisionExpr {
    Match(VisionMatcher),
    Any(Vec<VisionExpr>),
    First(Box<VisionExpr>),
    Nth(Box<VisionExpr>, usize),
    Nearest {
        anchor: Box<VisionExpr>,
        target: Box<VisionExpr>,
        direction: SpatialDirection,
        index: usize,
        metric: DistanceMetric,
    },
}

#[derive(Debug)]
struct VisionMatcher {
    predicates: Vec<VisionPredicate>,
    exact_seed: Option<String>,
}

#[derive(Debug)]
enum VisionPredicate {
    Text {
        operator: MatchOperator,
        expected: String,
    },
    Regex(Regex),
}

fn compile_expression(expression: &QueryExpr) -> Result<VisionExpr, VisionQueryCompileError> {
    match expression {
        QueryExpr::Match { matcher } if matcher.role == ElementRole::Text => {
            let mut predicates = Vec::with_capacity(matcher.predicates.len());
            let mut exact_seed = None;
            for predicate in &matcher.predicates {
                if predicate.attribute != SelectorAttribute::Name {
                    return unsupported("OCR text queries only expose the portable name property");
                }
                match (&predicate.operator, &predicate.value) {
                    (MatchOperator::Regex, PredicateValue::Regex(literal)) => {
                        let regex = RegexBuilder::new(&literal.pattern)
                            .case_insensitive(literal.case_insensitive)
                            .build()
                            .map_err(|error| VisionQueryCompileError {
                                message: format!("invalid prepared regex: {error}"),
                            })?;
                        predicates.push(VisionPredicate::Regex(regex));
                    }
                    (operator, PredicateValue::Text(value))
                        if *operator != MatchOperator::Regex =>
                    {
                        let expected = normalize_text(value);
                        if *operator == MatchOperator::Equal && exact_seed.is_none() {
                            exact_seed = Some(expected.clone());
                        }
                        predicates.push(VisionPredicate::Text {
                            operator: *operator,
                            expected,
                        });
                    }
                    _ => {
                        return unsupported(
                            "OCR name predicates require a resolved text or regex value",
                        );
                    }
                }
            }
            Ok(VisionExpr::Match(VisionMatcher {
                predicates,
                exact_seed,
            }))
        }
        QueryExpr::Match { .. } => unsupported("OCR scenes only contain text role nodes"),
        QueryExpr::Any { queries } => Ok(VisionExpr::Any(
            queries
                .iter()
                .map(compile_expression)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        QueryExpr::First { query } => Ok(VisionExpr::First(Box::new(compile_expression(query)?))),
        QueryExpr::Nth { query, index } => Ok(VisionExpr::Nth(
            Box::new(compile_expression(query)?),
            index.get(),
        )),
        QueryExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => Ok(VisionExpr::Nearest {
            anchor: Box::new(compile_expression(anchor)?),
            target: Box::new(compile_expression(target)?),
            direction: *direction,
            index: index.get(),
            metric: *metric,
        }),
        QueryExpr::Descendant { .. }
        | QueryExpr::Child { .. }
        | QueryExpr::Not { .. }
        | QueryExpr::Css { .. } => {
            unsupported("OCR scenes support text, any, first, nth, and nearest expressions only")
        }
    }
}

fn unsupported<T>(message: &str) -> Result<T, VisionQueryCompileError> {
    Err(VisionQueryCompileError {
        message: message.to_owned(),
    })
}

fn evaluate_expression<'scene>(
    scene: &'scene AppScene,
    expression: &VisionExpr,
    window_scope: &[usize],
    query_source: &str,
    metrics: &mut VisionQueryMetrics,
) -> Result<Vec<AppNodeRef<'scene>>, AutomationError> {
    match expression {
        VisionExpr::Match(matcher) => Ok(match_nodes(scene, matcher, window_scope, metrics)),
        VisionExpr::Any(branches) => {
            for branch in branches {
                let matches =
                    evaluate_expression(scene, branch, window_scope, query_source, metrics)?;
                if !matches.is_empty() {
                    return Ok(matches);
                }
            }
            Ok(Vec::new())
        }
        VisionExpr::First(query) => {
            Ok(
                evaluate_expression(scene, query, window_scope, query_source, metrics)?
                    .into_iter()
                    .take(1)
                    .collect(),
            )
        }
        VisionExpr::Nth(query, index) => {
            Ok(
                evaluate_expression(scene, query, window_scope, query_source, metrics)?
                    .into_iter()
                    .nth(index - 1)
                    .into_iter()
                    .collect(),
            )
        }
        VisionExpr::Nearest {
            anchor,
            target,
            direction,
            index,
            metric,
        } => evaluate_nearest(
            scene,
            anchor,
            target,
            *direction,
            *index,
            *metric,
            window_scope,
            query_source,
            metrics,
        ),
    }
}

fn match_nodes<'scene>(
    scene: &'scene AppScene,
    matcher: &VisionMatcher,
    window_scope: &[usize],
    metrics: &mut VisionQueryMetrics,
) -> Vec<AppNodeRef<'scene>> {
    let mut matches = Vec::new();
    for &window_index in window_scope {
        let window = &scene.windows[window_index];
        match matcher.exact_seed.as_deref() {
            Some(seed) => {
                let indices = window.scene.index().exact(seed);
                metrics.exact_index_hits += usize::from(!indices.is_empty());
                for &node_index in indices {
                    let node = &window.scene.nodes[node_index];
                    metrics.scanned_nodes += 1;
                    if matcher.matches(&node.normalized_text) {
                        matches.push(AppNodeRef {
                            window: &window.window,
                            scene: &window.scene,
                            node,
                        });
                    }
                }
            }
            None => {
                for node in &window.scene.nodes {
                    metrics.scanned_nodes += 1;
                    if matcher.matches(&node.normalized_text) {
                        matches.push(AppNodeRef {
                            window: &window.window,
                            scene: &window.scene,
                            node,
                        });
                    }
                }
            }
        }
    }
    matches
}

impl VisionMatcher {
    fn matches(&self, actual: &str) -> bool {
        self.predicates.iter().all(|predicate| match predicate {
            VisionPredicate::Text { operator, expected } => match operator {
                MatchOperator::Equal => actual == expected,
                MatchOperator::NotEqual => actual != expected,
                MatchOperator::Contains => actual.contains(expected),
                MatchOperator::StartsWith => actual.starts_with(expected),
                MatchOperator::EndsWith => actual.ends_with(expected),
                MatchOperator::Regex => false,
            },
            VisionPredicate::Regex(regex) => regex.is_match(actual),
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_nearest<'scene>(
    scene: &'scene AppScene,
    anchor: &VisionExpr,
    target: &VisionExpr,
    direction: SpatialDirection,
    index: usize,
    metric: DistanceMetric,
    window_scope: &[usize],
    query_source: &str,
    metrics: &mut VisionQueryMetrics,
) -> Result<Vec<AppNodeRef<'scene>>, AutomationError> {
    let anchors = evaluate_expression(scene, anchor, window_scope, query_source, metrics)?;
    let anchor = match anchors.as_slice() {
        [] => return Ok(Vec::new()),
        [anchor] => *anchor,
        matches => {
            return Err(AutomationError::AmbiguousTarget {
                query: query_source.to_owned(),
                matches: matches.len(),
                details: "nearest anchor must resolve to exactly one OCR text node".to_owned(),
            });
        }
    };
    let anchor_window_index = scene
        .windows
        .iter()
        .position(|window| window.window.identity == anchor.window.identity)
        .ok_or_else(|| AutomationError::BackendFailed {
            backend: argusflow_core::BackendKind::OcrSmall,
            message: "nearest anchor window disappeared from the frozen scene".to_owned(),
        })?;
    let mut ranked = BTreeMap::<u128, Vec<AppNodeRef<'scene>>>::new();
    for candidate in
        evaluate_expression(scene, target, &[anchor_window_index], query_source, metrics)?
    {
        if candidate.node.id == anchor.node.id
            || !in_direction(anchor.node.bbox, candidate.node.bbox, direction)
        {
            continue;
        }
        metrics.spatial_candidates += 1;
        ranked
            .entry(distance_squared(
                anchor.node.bbox,
                candidate.node.bbox,
                metric,
            ))
            .or_default()
            .push(candidate);
    }
    let Some((_, selected_rank)) = ranked.into_iter().nth(index - 1) else {
        return Ok(Vec::new());
    };
    if selected_rank.len() > 1 {
        return Err(AutomationError::AmbiguousTarget {
            query: query_source.to_owned(),
            matches: selected_rank.len(),
            details: format!("nearest distance rank {index} contains an exact geometry tie"),
        });
    }
    Ok(selected_rank)
}

fn in_direction(anchor: PhysicalRect, target: PhysicalRect, direction: SpatialDirection) -> bool {
    let anchor_x = i128::from(anchor.x) * 2 + i128::from(anchor.width);
    let anchor_y = i128::from(anchor.y) * 2 + i128::from(anchor.height);
    let target_x = i128::from(target.x) * 2 + i128::from(target.width);
    let target_y = i128::from(target.y) * 2 + i128::from(target.height);
    match direction {
        SpatialDirection::Any => true,
        SpatialDirection::Above => target_y < anchor_y,
        SpatialDirection::Below => target_y > anchor_y,
        SpatialDirection::Left => target_x < anchor_x,
        SpatialDirection::Right => target_x > anchor_x,
    }
}

fn distance_squared(anchor: PhysicalRect, target: PhysicalRect, metric: DistanceMetric) -> u128 {
    let (dx, dy) = match metric {
        DistanceMetric::CenterDistanceNormalized => {
            let anchor_x = i128::from(anchor.x) * 2 + i128::from(anchor.width);
            let anchor_y = i128::from(anchor.y) * 2 + i128::from(anchor.height);
            let target_x = i128::from(target.x) * 2 + i128::from(target.width);
            let target_y = i128::from(target.y) * 2 + i128::from(target.height);
            (
                (target_x - anchor_x).unsigned_abs(),
                (target_y - anchor_y).unsigned_abs(),
            )
        }
        DistanceMetric::EdgeGapNormalized => {
            let horizontal = if anchor.right() < i64::from(target.x) {
                i128::from(i64::from(target.x) - anchor.right())
            } else if target.right() < i64::from(anchor.x) {
                i128::from(i64::from(anchor.x) - target.right())
            } else {
                0
            };
            let vertical = if anchor.bottom() < i64::from(target.y) {
                i128::from(i64::from(target.y) - anchor.bottom())
            } else if target.bottom() < i64::from(anchor.y) {
                i128::from(i64::from(anchor.y) - target.bottom())
            } else {
                0
            };
            (horizontal as u128, vertical as u128)
        }
    };
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
}

#[cfg(test)]
#[path = "aql/tests.rs"]
mod tests;
