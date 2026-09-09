//! 有界的八邻域文本连通域，提取外边界凸包供最小旋转矩形使用。
use crate::OcrError as Failure;
use argusflow_core::{FailureKind, Operation};
use geo::{ConvexHull, MultiPoint, Point, Polygon};
use std::collections::VecDeque;

pub(crate) fn boundaries(
    values: &[f32],
    width: usize,
    height: usize,
    threshold: f32,
    max_candidates: usize,
    operation: &Operation,
) -> Result<Vec<Polygon>, Failure> {
    let mut visited = vec![false; values.len()];
    let mut queue = VecDeque::new();
    let mut result = Vec::new();
    let mut candidates = 0;
    for start in 0..values.len() {
        if start % 4096 == 0 {
            operation.check("db_components")?;
        }
        if visited[start] || values[start] <= threshold {
            continue;
        }
        candidates += 1;
        if candidates > max_candidates {
            return Err(limit("检测候选数量超限"));
        }
        visited[start] = true;
        queue.push_back(start);
        let mut border = Vec::new();
        let mut processed = 0usize;
        while let Some(index) = queue.pop_front() {
            processed += 1;
            if processed.is_multiple_of(1024) {
                operation.check("db_component")?;
            }
            let x = index % width;
            let y = index / width;
            let mut edge = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
            for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = ny * width + nx;
                    if values[neighbor] <= threshold {
                        edge = true;
                    } else if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }
            }
            if edge {
                if border.len() >= 200_000 {
                    return Err(limit("单个文本轮廓边界超过 200000 个点"));
                }
                border.push(Point::new(x as f64, y as f64));
            }
        }
        if border.len() >= 4 {
            result.push(MultiPoint(border).convex_hull());
        }
    }
    Ok(result)
}
fn limit(message: &str) -> Failure {
    Failure::new(FailureKind::ResourceLimit, "db_candidates", message)
}

#[cfg(test)]
#[path = "../../../../../tests/argusflow-vision/unit/ocr/detection/components.rs"]
mod tests;
