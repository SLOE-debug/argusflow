//! 所有平台共用几何计算；预览与执行使用同一候选筛选和排序结果。
use super::{Geometry, QueryTree, Rect};
use crate::{Length, LengthUnit, Operand, SpatialOptions, SpatialOrder, Value};
use argusflow_core::{Failure, FailureKind, Operation};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
/// 候选计算结果及排除理由。
pub struct SpatialCandidate {
    /// 快照节点索引。
    pub node: usize,
    /// 候选外框。
    pub bounds: Rect,
    /// 逆时针角度；中心重合时缺失。
    pub angle: Option<f64>,
    /// 当前坐标单位中的距离。
    pub distance: f64,
    /// 被排除的原因。
    pub reason: Option<String>,
    /// 筛选后的排序序号。
    pub rank: Option<usize>,
    /// 是否最终选中。
    pub selected: bool,
}
#[derive(Debug, Clone, Serialize)]
/// 与执行同源的空间定位解释，不代表动作已执行。
pub struct SpatialPreview {
    /// 坐标空间身份。
    pub space: String,
    /// 查找范围。
    pub scope: Rect,
    /// 唯一锚点外框。
    pub anchor: Rect,
    /// 查找扇形。
    pub angles: Option<(f64, f64)>,
    /// 包括排除项的全部候选。
    pub candidates: Vec<SpatialCandidate>,
}
pub(super) fn geometry<T>(tree: &QueryTree<T>, index: usize) -> Result<&Geometry, Failure> {
    tree.node(index).and_then(|n| n.geometry()).ok_or_else(|| {
        error(
            FailureKind::Unsupported,
            "候选或锚点缺少可靠位置，无法执行空间条件",
        )
    })
}
fn error(kind: FailureKind, message: &str) -> Failure {
    Failure::new(kind, "aql_spatial", message)
}
pub(super) fn unique(indices: &[usize], label: &str) -> Result<usize, Failure> {
    if indices.len() != 1 {
        return Err(error(
            if indices.is_empty() {
                FailureKind::NotFound
            } else {
                FailureKind::Ambiguous
            },
            &format!("{label}需要唯一目标，实际找到 {} 个", indices.len()),
        ));
    }
    Ok(indices[0])
}
fn same_space(a: &Geometry, b: &Geometry) -> Result<(), Failure> {
    if a.space != b.space || a.scope != b.scope || a.pixels_per_logical != b.pixels_per_logical {
        return Err(error(
            FailureKind::Unsupported,
            "锚点和候选不属于同一已确认坐标空间及单位",
        ));
    }
    Ok(())
}
fn length(length: &Length, geometry: &Geometry, scope: Rect) -> Result<f64, Failure> {
    let Operand::Literal(Value::Number(value)) = length.value else {
        return Err(error(FailureKind::InvalidInput, "空间长度参数未绑定"));
    };
    if !value.is_finite() || value < 0.0 {
        return Err(error(FailureKind::InvalidInput, "空间长度必须为有限非负数"));
    }
    let factor = match length.unit {
        LengthUnit::LogicalPixels => geometry.pixels_per_logical.ok_or_else(|| {
            error(
                FailureKind::Unsupported,
                "来源无法可靠转换逻辑像素，请使用范围百分比",
            )
        })?,
        LengthUnit::ScopeShort => scope.0[2].min(scope.0[3]) / 100.0,
        LengthUnit::ScopeWidth => scope.0[2] / 100.0,
        LengthUnit::ScopeHeight => scope.0[3] / 100.0,
    };
    let result = value * factor;
    if !result.is_finite() || result > 1e12 {
        return Err(error(FailureKind::InvalidInput, "空间长度超过计算预算"));
    }
    Ok(result)
}
fn key(candidate: &SpatialCandidate, order: SpatialOrder) -> [f64; 2] {
    let [x, y] = candidate.bounds.center();
    match order {
        SpatialOrder::Near => [candidate.distance, 0.0],
        SpatialOrder::Far => [-candidate.distance, 0.0],
        SpatialOrder::Top => [y, 0.0],
        SpatialOrder::Left => [x, 0.0],
        SpatialOrder::TopLeft => [y, x],
    }
}
fn compare(
    a: &SpatialCandidate,
    b: &SpatialCandidate,
    order: SpatialOrder,
    secondary: Option<SpatialOrder>,
) -> std::cmp::Ordering {
    let cmp = |a: [f64; 2], b: [f64; 2]| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]));
    cmp(key(a, order), key(b, order))
        .then_with(|| secondary.map_or(std::cmp::Ordering::Equal, |o| cmp(key(a, o), key(b, o))))
}
fn select(rows: &mut [SpatialCandidate], options: &SpatialOptions) -> Result<Vec<usize>, Failure> {
    let mut indices = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.reason.is_none())
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if let Some(order) = options.order {
        indices.sort_by(|a, b| compare(&rows[*a], &rows[*b], order, options.secondary));
    }
    for (rank, &index) in indices.iter().enumerate() {
        rows[index].rank = Some(rank + 1);
    }
    if let Some(rank) = options.rank {
        let index = rank
            .checked_sub(1)
            .and_then(|rank| indices.get(rank))
            .copied()
            .ok_or_else(|| error(FailureKind::NotFound, &format!("没有第 {rank} 个匹配目标")))?;
        if let Some(order) = options.order
            && indices.iter().any(|&other| {
                other != index
                    && compare(&rows[index], &rows[other], order, options.secondary).is_eq()
            })
        {
            return Err(error(
                FailureKind::Ambiguous,
                "候选排序键相同，请增加次排序或缩小查找范围",
            ));
        }
        indices = vec![index];
    }
    for &index in &indices {
        rows[index].selected = true;
    }
    Ok(indices.into_iter().map(|i| rows[i].node).collect())
}
pub(super) fn position<T>(
    tree: &QueryTree<T>,
    candidates: &[usize],
    order: SpatialOrder,
    rank: usize,
    operation: &Operation,
) -> Result<Vec<usize>, Failure> {
    let mut rows = Vec::new();
    let mut first = None;
    for &node in candidates {
        operation.check("aql_position")?;
        let g = geometry(tree, node)?;
        if let Some(first) = first {
            same_space(first, g)?;
        } else {
            first = Some(g);
        }
        rows.push(SpatialCandidate {
            node,
            bounds: g.bounds,
            angle: None,
            distance: 0.0,
            reason: None,
            rank: None,
            selected: false,
        });
    }
    select(
        &mut rows,
        &SpatialOptions {
            order: Some(order),
            rank: Some(rank),
            ..Default::default()
        },
    )
}
pub(super) fn filter<T>(
    tree: &QueryTree<T>,
    anchor: usize,
    candidates: &[usize],
    region: Option<usize>,
    second: Option<usize>,
    options: &SpatialOptions,
    operation: &Operation,
) -> Result<(Vec<usize>, SpatialPreview), Failure> {
    let origin = geometry(tree, anchor)?;
    let region = region.map(|i| geometry(tree, i)).transpose()?;
    let second = second.map(|i| geometry(tree, i)).transpose()?;
    for other in [region, second].into_iter().flatten() {
        same_space(origin, other)?;
    }
    if second.is_some_and(|g| g.bounds.center() == origin.bounds.center()) {
        return Err(error(FailureKind::InvalidInput, "两锚点的中心不能重合"));
    }
    let scope = region.map_or(origin.scope, |g| g.bounds);
    let min = options
        .min_distance
        .as_ref()
        .map(|v| length(v, origin, scope))
        .transpose()?
        .unwrap_or(0.0);
    let max = options
        .max_distance
        .as_ref()
        .map(|v| length(v, origin, scope))
        .transpose()?
        .unwrap_or(f64::INFINITY);
    if min > max {
        return Err(error(FailureKind::InvalidInput, "最小距离不能大于最大距离"));
    }
    let tolerance = options
        .tolerance
        .as_ref()
        .map(|v| length(v, origin, scope))
        .transpose()?
        .unwrap_or(0.0);
    let bandwidth = options
        .bandwidth
        .as_ref()
        .map(|v| length(v, origin, scope))
        .transpose()?
        .unwrap_or(0.0);
    let [ax, ay] = origin.bounds.center();
    let mut rows = Vec::new();
    for &node in candidates {
        operation.check("aql_spatial_filter")?;
        let g = geometry(tree, node)?;
        same_space(origin, g)?;
        let [x, y] = g.bounds.center();
        let dx = x - ax;
        let dy = ay - y;
        let angle = if dx == 0.0 && dy == 0.0 {
            None
        } else {
            Some(dy.atan2(dx).to_degrees().rem_euclid(360.0))
        };
        let distance = if options.edge_distance {
            origin.bounds.edge_distance(g.bounds)
        } else {
            dx.hypot(dy)
        };
        let reason = if node == anchor {
            Some("锚点自身")
        } else if region.is_some()
            && (if options.outside {
                scope.overlaps(g.bounds)
            } else {
                !scope.contains(g.bounds)
            })
        {
            Some("不在指定区域")
        } else if options.angles.is_some_and(|(start, end)| {
            !angle.is_some_and(|a| {
                if start < end {
                    a >= start && a < end
                } else {
                    a >= start || a < end
                }
            })
        }) {
            Some("不在角度区间或中心重合")
        } else if distance < min || distance > max {
            Some("不在距离范围")
        } else if options.row.is_some_and(|row| {
            if row {
                dy.abs() > tolerance
            } else {
                dx.abs() > tolerance
            }
        }) {
            Some("超出对齐容差")
        } else if options.exclude_overlap && origin.bounds.overlaps(g.bounds) {
            Some("与锚点重叠")
        } else if second.is_some_and(|other| {
            !between(
                [ax, ay],
                other.bounds.center(),
                [x, y],
                options.between_rectangle,
                bandwidth,
            )
        }) {
            Some("不在两锚点之间")
        } else {
            None
        };
        rows.push(SpatialCandidate {
            node,
            bounds: g.bounds,
            angle,
            distance,
            reason: reason.map(str::to_owned),
            rank: None,
            selected: false,
        });
    }
    let result = select(&mut rows, options)?;
    Ok((
        result,
        SpatialPreview {
            space: origin.space.clone(),
            scope,
            anchor: origin.bounds,
            angles: options.angles,
            candidates: rows,
        },
    ))
}
fn between(a: [f64; 2], b: [f64; 2], p: [f64; 2], rectangle: bool, width: f64) -> bool {
    if rectangle {
        return p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1]);
    }
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let squared = dx * dx + dy * dy;
    if squared == 0.0 {
        return false;
    }
    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / squared;
    (0.0..=1.0).contains(&t) && (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy) <= width / 2.0
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-aql/unit/spatial.rs"]
mod tests;
