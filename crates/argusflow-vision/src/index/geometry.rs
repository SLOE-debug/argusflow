//! 分辨率无关的视觉矩形关系与距离。

use argusflow_core::SpatialDirection;

use crate::{frame::PhysicalRect, scene::VisualNode};

/// 判断目标是否满足 anchor-relative 方向；容差按局部文字高度计算。
pub fn direction_matches(
    anchor: &VisualNode,
    target: &VisualNode,
    direction: SpatialDirection,
) -> bool {
    let (anchor_x, anchor_y) = anchor.center();
    let (target_x, target_y) = target.center();
    let overlap_tolerance = anchor.bbox.height.min(target.bbox.height) as f32 * 0.25;
    match direction {
        SpatialDirection::Any => true,
        SpatialDirection::Above => {
            target_y <= anchor_y && vertical_gap(target.bbox, anchor.bbox) >= -overlap_tolerance
        }
        SpatialDirection::Below => {
            target_y >= anchor_y && vertical_gap(anchor.bbox, target.bbox) >= -overlap_tolerance
        }
        SpatialDirection::Left => {
            target_x <= anchor_x && horizontal_gap(target.bbox, anchor.bbox) >= -overlap_tolerance
        }
        SpatialDirection::Right => {
            target_x >= anchor_x && horizontal_gap(anchor.bbox, target.bbox) >= -overlap_tolerance
        }
    }
}

/// 返回矩形边缘间隙除以 viewport 对角线的距离。
pub fn edge_gap_normalized(
    anchor: PhysicalRect,
    target: PhysicalRect,
    viewport: PhysicalRect,
) -> f32 {
    let horizontal = axis_gap(
        i64::from(anchor.x),
        anchor.right(),
        i64::from(target.x),
        target.right(),
    );
    let vertical = axis_gap(
        i64::from(anchor.y),
        anchor.bottom(),
        i64::from(target.y),
        target.bottom(),
    );
    normalize_distance(horizontal.hypot(vertical), viewport)
}

/// 返回矩形中心欧氏距离除以 viewport 对角线的距离。
pub fn center_distance_normalized(
    anchor: PhysicalRect,
    target: PhysicalRect,
    viewport: PhysicalRect,
) -> f32 {
    let anchor_x = anchor.x as f32 + anchor.width as f32 / 2.0;
    let anchor_y = anchor.y as f32 + anchor.height as f32 / 2.0;
    let target_x = target.x as f32 + target.width as f32 / 2.0;
    let target_y = target.y as f32 + target.height as f32 / 2.0;
    normalize_distance((target_x - anchor_x).hypot(target_y - anchor_y), viewport)
}

/// 返回两个闭区间边界的非负间隙，相交时为零。
fn axis_gap(left_start: i64, left_end: i64, right_start: i64, right_end: i64) -> f32 {
    if left_end < right_start {
        (right_start - left_end) as f32
    } else if right_end < left_start {
        (left_start - right_end) as f32
    } else {
        0.0
    }
}

/// 将物理距离除以当前 viewport 对角线。
fn normalize_distance(distance: f32, viewport: PhysicalRect) -> f32 {
    let diagonal = (viewport.width as f32).hypot(viewport.height as f32);
    if diagonal > 0.0 {
        distance / diagonal
    } else {
        f32::INFINITY
    }
}

/// 返回上下矩形的有符号垂直间隙。
fn vertical_gap(above: PhysicalRect, below: PhysicalRect) -> f32 {
    i64::from(below.y).saturating_sub(above.bottom()) as f32
}

/// 返回左右矩形的有符号水平间隙。
fn horizontal_gap(left: PhysicalRect, right: PhysicalRect) -> f32 {
    i64::from(right.x).saturating_sub(left.right()) as f32
}
