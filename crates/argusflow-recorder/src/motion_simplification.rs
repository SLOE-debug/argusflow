//! 鼠标轨迹几何简化，保留回折与时间锚点，避免匀速长线或微抖重复占用 JSON。

use crate::MotionPoint;
use argusflow_core::ScreenPoint;

/// 最多每隔 250ms 保留一个真实时间锚点，轨迹仍能表达速度变化和持续时间。
const TIME_ANCHOR_MS: u64 = 250;
/// 物理像素误差平方；在原始坐标空间判断，与显示缩放无关。
const MAX_ERROR_SQUARED: f64 = 4.0;

pub(crate) fn simplify(points: Vec<MotionPoint>) -> Vec<MotionPoint> {
    if points.len() <= 2 {
        return points;
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    // 时间锚点先切开局部问题，限制高采样率下的简化计算量。
    let mut anchor = 0;
    for index in 1..points.len() {
        if index + 1 == points.len()
            || points[index]
                .elapsed_ms
                .saturating_sub(points[anchor].elapsed_ms)
                >= TIME_ANCHOR_MS
        {
            keep[index] = true;
            simplify_span(&points, anchor, index, &mut keep);
            anchor = index;
        }
    }
    points
        .into_iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(point))
        .collect()
}

/// 迭代 RDP，避免异常折线递归过深；到线段的距离可以保留折返与闭环。
fn simplify_span(points: &[MotionPoint], first: usize, last: usize, keep: &mut [bool]) {
    let mut pending = vec![(first, last)];
    while let Some((start, end)) = pending.pop() {
        let mut furthest = None;
        let mut max_distance = MAX_ERROR_SQUARED;
        for index in start + 1..end {
            let distance = segment_distance_squared(
                points[index].point,
                points[start].point,
                points[end].point,
            );
            if distance > max_distance {
                furthest = Some(index);
                max_distance = distance;
            }
        }
        if let Some(index) = furthest {
            keep[index] = true;
            pending.push((start, index));
            pending.push((index, end));
        }
    }
}

/// 先转为 f64 再相减，极端虚拟桌面坐标也不发生 i32 溢出。
fn segment_distance_squared(point: ScreenPoint, start: ScreenPoint, end: ScreenPoint) -> f64 {
    let dx = f64::from(end.x) - f64::from(start.x);
    let dy = f64::from(end.y) - f64::from(start.y);
    let px = f64::from(point.x) - f64::from(start.x);
    let py = f64::from(point.y) - f64::from(start.y);
    let length_squared = dx * dx + dy * dy;
    let progress = if length_squared == 0.0 {
        0.0
    } else {
        ((px * dx + py * dy) / length_squared).clamp(0.0, 1.0)
    };
    (px - progress * dx).powi(2) + (py - progress * dy).powi(2)
}
