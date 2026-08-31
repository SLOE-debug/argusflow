//! Vision AQL 的元素与 viewport 空间锚点求值。

use std::collections::BTreeMap;

use argusflow_core::{
    AutomationError, BackendKind, DistanceMetric, SpatialAnchor, SpatialDirection, ViewportCorner,
    ViewportEdge,
};

use super::{VisionExpr, VisionQueryCompileError, evaluate_expression, unsupported};
use crate::{AppNodeRef, AppScene, PhysicalRect, VisionQueryMetrics};

/// Vision 编译后不再依赖 AQL AST 的空间锚点。
#[derive(Debug)]
pub(super) enum VisionAnchor {
    /// 必须唯一求值的元素锚点。
    Element(Box<VisionExpr>),
    /// 每个候选窗口自己的 viewport 角。
    ViewportCorner(ViewportCorner),
    /// 每个候选窗口自己的 viewport 边。
    ViewportEdge(ViewportEdge),
}

/// 编译元素查询或无坐标 viewport 锚点。
pub(super) fn compile_anchor(
    anchor: &SpatialAnchor,
) -> Result<VisionAnchor, VisionQueryCompileError> {
    Ok(match anchor {
        SpatialAnchor::Element { query } => {
            VisionAnchor::Element(Box::new(super::compile_expression(query)?))
        }
        SpatialAnchor::ViewportCorner { position } => VisionAnchor::ViewportCorner(*position),
        SpatialAnchor::ViewportEdge { side } => VisionAnchor::ViewportEdge(*side),
    })
}

/// 执行元素到元素或 viewport 到元素的严格空间排名。
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_nearest<'scene>(
    scene: &'scene AppScene,
    anchor: &VisionAnchor,
    target: &VisionExpr,
    direction: SpatialDirection,
    index: usize,
    metric: DistanceMetric,
    window_scope: &[usize],
    query_source: &str,
    metrics: &mut VisionQueryMetrics,
) -> Result<Vec<AppNodeRef<'scene>>, AutomationError> {
    match anchor {
        VisionAnchor::Element(anchor) => evaluate_from_element(
            scene,
            anchor,
            target,
            direction,
            index,
            metric,
            window_scope,
            query_source,
            metrics,
        ),
        VisionAnchor::ViewportCorner(position) => evaluate_from_viewports(
            scene,
            target,
            ViewportAnchor::Corner(*position),
            index,
            metric,
            window_scope,
            query_source,
            metrics,
        ),
        VisionAnchor::ViewportEdge(side) => evaluate_from_viewports(
            scene,
            target,
            ViewportAnchor::Edge(*side),
            index,
            metric,
            window_scope,
            query_source,
            metrics,
        ),
    }
}

/// 元素锚点必须全局唯一，目标随后只在锚点所属窗口排名。
#[allow(clippy::too_many_arguments)]
fn evaluate_from_element<'scene>(
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
            backend: BackendKind::OcrSmall,
            message: "nearest anchor window disappeared from the frozen scene".to_owned(),
        })?;
    let viewport = scene.windows[anchor_window_index].scene.viewport;
    let candidates =
        evaluate_expression(scene, target, &[anchor_window_index], query_source, metrics)?
            .into_iter()
            .filter(|candidate| {
                candidate.node.id != anchor.node.id
                    && in_direction(anchor.node.bbox, candidate.node.bbox, direction)
            });
    select_ranked(
        candidates,
        |candidate| distance_from_rect(anchor.node.bbox, candidate.node.bbox, metric, viewport),
        index,
        query_source,
        metrics,
    )
    .map(|selected| selected.into_iter().collect())
}

/// viewport 锚点逐窗口排名；多个窗口产生结果时保留多个候选交给唯一性规则拒绝。
#[allow(clippy::too_many_arguments)]
fn evaluate_from_viewports<'scene>(
    scene: &'scene AppScene,
    target: &VisionExpr,
    anchor: ViewportAnchor,
    index: usize,
    metric: DistanceMetric,
    window_scope: &[usize],
    query_source: &str,
    metrics: &mut VisionQueryMetrics,
) -> Result<Vec<AppNodeRef<'scene>>, AutomationError> {
    let mut selected = Vec::new();
    for &window_index in window_scope {
        let viewport = scene.windows[window_index].scene.viewport;
        let candidates =
            evaluate_expression(scene, target, &[window_index], query_source, metrics)?;
        if let Some(candidate) = select_ranked(
            candidates,
            |candidate| distance_from_viewport(anchor, candidate.node.bbox, metric, viewport),
            index,
            query_source,
            metrics,
        )? {
            selected.push(candidate);
        }
    }
    Ok(selected)
}

/// viewport 锚点的封闭集合。
#[derive(Debug, Clone, Copy)]
enum ViewportAnchor {
    /// viewport 角点。
    Corner(ViewportCorner),
    /// viewport 边线。
    Edge(ViewportEdge),
}

/// 按真实候选序号选择结果；请求序号落入同距组时严格报歧义。
fn select_ranked<'scene>(
    candidates: impl IntoIterator<Item = AppNodeRef<'scene>>,
    distance: impl Fn(AppNodeRef<'scene>) -> u128,
    index: usize,
    query_source: &str,
    metrics: &mut VisionQueryMetrics,
) -> Result<Option<AppNodeRef<'scene>>, AutomationError> {
    let mut ranked = BTreeMap::<u128, Vec<AppNodeRef<'scene>>>::new();
    for candidate in candidates {
        metrics.spatial_candidates += 1;
        ranked
            .entry(distance(candidate))
            .or_default()
            .push(candidate);
    }
    let mut preceding = 0_usize;
    for candidates_at_distance in ranked.into_values() {
        let rank_end = preceding.saturating_add(candidates_at_distance.len());
        if index <= rank_end {
            if candidates_at_distance.len() > 1 {
                return Err(AutomationError::AmbiguousTarget {
                    query: query_source.to_owned(),
                    matches: candidates_at_distance.len(),
                    details: format!(
                        "nearest candidate rank {index} belongs to an exact geometry tie"
                    ),
                });
            }
            return Ok(candidates_at_distance.into_iter().next());
        }
        preceding = rank_end;
    }
    Ok(None)
}

/// 判断元素候选是否位于元素锚点指定方向。
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

/// 计算两个元素矩形间按 viewport 两轴分别归一化的平方距离键。
fn distance_from_rect(
    anchor: PhysicalRect,
    target: PhysicalRect,
    metric: DistanceMetric,
    viewport: PhysicalRect,
) -> u128 {
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
        DistanceMetric::EdgeGapNormalized => edge_gaps(anchor, target),
    };
    normalized_distance_key(dx, dy, viewport)
}

/// 计算候选矩形到当前 viewport 角或边的归一化平方距离键。
fn distance_from_viewport(
    anchor: ViewportAnchor,
    target: PhysicalRect,
    metric: DistanceMetric,
    viewport: PhysicalRect,
) -> u128 {
    let (dx, dy) = match (anchor, metric) {
        (ViewportAnchor::Corner(position), DistanceMetric::EdgeGapNormalized) => {
            let (x, y) = corner_point(viewport, position);
            point_to_rect_gaps(x, y, target)
        }
        (ViewportAnchor::Corner(position), DistanceMetric::CenterDistanceNormalized) => {
            let (x, y) = corner_point(viewport, position);
            let target_x = i128::from(target.x) * 2 + i128::from(target.width);
            let target_y = i128::from(target.y) * 2 + i128::from(target.height);
            (
                (target_x - x * 2).unsigned_abs(),
                (target_y - y * 2).unsigned_abs(),
            )
        }
        (ViewportAnchor::Edge(side), DistanceMetric::EdgeGapNormalized) => {
            edge_to_rect_gap(viewport, target, side)
        }
        (ViewportAnchor::Edge(side), DistanceMetric::CenterDistanceNormalized) => {
            edge_to_center_gap(viewport, target, side)
        }
    };
    normalized_distance_key(dx, dy, viewport)
}

/// 返回两个矩形在水平和垂直轴上的非重叠间隙。
fn edge_gaps(anchor: PhysicalRect, target: PhysicalRect) -> (u128, u128) {
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

/// 返回 viewport 角点的帧内坐标。
fn corner_point(viewport: PhysicalRect, position: ViewportCorner) -> (i128, i128) {
    let left = i128::from(viewport.x);
    let top = i128::from(viewport.y);
    let right = i128::from(viewport.right());
    let bottom = i128::from(viewport.bottom());
    match position {
        ViewportCorner::TopLeft => (left, top),
        ViewportCorner::TopRight => (right, top),
        ViewportCorner::BottomLeft => (left, bottom),
        ViewportCorner::BottomRight => (right, bottom),
    }
}

/// 返回点到矩形的两轴间隙。
fn point_to_rect_gaps(x: i128, y: i128, target: PhysicalRect) -> (u128, u128) {
    let left = i128::from(target.x);
    let top = i128::from(target.y);
    let right = i128::from(target.right());
    let bottom = i128::from(target.bottom());
    let dx = if x < left {
        left - x
    } else if x > right {
        x - right
    } else {
        0
    };
    let dy = if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0
    };
    (dx as u128, dy as u128)
}

/// 返回 viewport 边到候选矩形的垂直间隙。
fn edge_to_rect_gap(
    viewport: PhysicalRect,
    target: PhysicalRect,
    side: ViewportEdge,
) -> (u128, u128) {
    match side {
        ViewportEdge::Top => (
            0,
            (i128::from(target.y) - i128::from(viewport.y)).unsigned_abs(),
        ),
        ViewportEdge::Right => (
            i128::from(viewport.right() - target.right()).unsigned_abs(),
            0,
        ),
        ViewportEdge::Bottom => (
            0,
            i128::from(viewport.bottom() - target.bottom()).unsigned_abs(),
        ),
        ViewportEdge::Left => (
            (i128::from(target.x) - i128::from(viewport.x)).unsigned_abs(),
            0,
        ),
    }
}

/// 返回 viewport 边到候选中心的垂直距离，中心坐标保持二倍整数。
fn edge_to_center_gap(
    viewport: PhysicalRect,
    target: PhysicalRect,
    side: ViewportEdge,
) -> (u128, u128) {
    let center_x = i128::from(target.x) * 2 + i128::from(target.width);
    let center_y = i128::from(target.y) * 2 + i128::from(target.height);
    match side {
        ViewportEdge::Top => (0, (center_y - i128::from(viewport.y) * 2).unsigned_abs()),
        ViewportEdge::Right => (
            (i128::from(viewport.right()) * 2 - center_x).unsigned_abs(),
            0,
        ),
        ViewportEdge::Bottom => (
            0,
            (i128::from(viewport.bottom()) * 2 - center_y).unsigned_abs(),
        ),
        ViewportEdge::Left => ((center_x - i128::from(viewport.x) * 2).unsigned_abs(), 0),
    }
}

/// 以共同分母的整数分子比较 `(dx / width)^2 + (dy / height)^2`。
fn normalized_distance_key(dx: u128, dy: u128, viewport: PhysicalRect) -> u128 {
    let width = u128::from(viewport.width);
    let height = u128::from(viewport.height);
    dx.saturating_mul(dx)
        .saturating_mul(height.saturating_mul(height))
        .saturating_add(
            dy.saturating_mul(dy)
                .saturating_mul(width.saturating_mul(width)),
        )
}

/// 防止 viewport anchor 的无方向约束绕过 Parser 进入编译计划。
pub(super) fn validate_viewport_direction(
    anchor: &SpatialAnchor,
    direction: SpatialDirection,
) -> Result<(), VisionQueryCompileError> {
    if !matches!(anchor, SpatialAnchor::Element { .. }) && direction != SpatialDirection::Any {
        return unsupported("viewport anchors require direction=any");
    }
    Ok(())
}
