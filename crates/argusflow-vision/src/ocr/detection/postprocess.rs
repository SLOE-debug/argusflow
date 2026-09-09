//! DB 概率图到原图四边形：轮廓、得分、外扩和最小旋转矩形。
use crate::OcrConfig;
use crate::OcrError as Failure;
use argusflow_core::{FailureKind, ImagePoint, Operation};
use geo::{Area, Buffer, Contains, Coord, MinimumRotatedRect, Point, Polygon};

pub(crate) struct Parameters {
    pub(crate) threshold: f32,
    pub(crate) box_threshold: f32,
    pub(crate) unclip: f64,
}

pub(crate) fn boxes(
    values: &[f32],
    width: usize,
    height: usize,
    original: (u32, u32),
    config: &OcrConfig,
    operation: &Operation,
    parameters: Parameters,
) -> Result<Vec<[ImagePoint; 4]>, Failure> {
    if width == 0
        || height == 0
        || width.checked_mul(height) != Some(values.len())
        || width > 4000
        || height > 4000
        || values
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(Failure::new(
            FailureKind::Protocol,
            "db_decode",
            "检测输出尺寸或数值无效",
        ));
    }
    let contours = super::components::boundaries(
        values,
        width,
        height,
        parameters.threshold,
        config.max_candidates,
        operation,
    )?;
    let mut result = Vec::new();
    for polygon in contours {
        operation.check("db_candidate")?;
        let Some(rectangle) = polygon.minimum_rotated_rect() else {
            continue;
        };
        let Some(rect) = ordered_rectangle(&rectangle) else {
            continue;
        };
        let side = |a: Coord, b: Coord| (a.x - b.x).hypot(a.y - b.y);
        if side(rect[0], rect[1]).min(side(rect[1], rect[2])) < 3.0 {
            continue;
        }
        if score(&rectangle, values, width, height, operation)? < parameters.box_threshold {
            continue;
        }
        let perimeter: f64 = rectangle
            .exterior()
            .lines()
            .map(|line| (line.start.x - line.end.x).hypot(line.start.y - line.end.y))
            .sum();
        if perimeter <= 0.0 {
            continue;
        }
        let expanded = rectangle.buffer(rectangle.unsigned_area() * parameters.unclip / perimeter);
        if expanded.0.len() != 1 {
            continue;
        }
        let Some(rectangle) = expanded.0[0].minimum_rotated_rect() else {
            continue;
        };
        let Some(rect) = ordered_rectangle(&rectangle) else {
            continue;
        };
        if side(rect[0], rect[1]).min(side(rect[1], rect[2])) < 5.0 {
            continue;
        }
        let source = if original.0 + original.1 < 64 {
            (original.0.max(32), original.1.max(32))
        } else {
            original
        };
        let polygon = rect.map(|point| ImagePoint {
            x: (point.x as f32 / width as f32 * source.0 as f32)
                .clamp(0.0, original.0.saturating_sub(1) as f32),
            y: (point.y as f32 / height as f32 * source.1 as f32)
                .clamp(0.0, original.1.saturating_sub(1) as f32),
        });
        if (polygon[1].x - polygon[0].x).hypot(polygon[1].y - polygon[0].y) >= 3.0
            && (polygon[3].x - polygon[0].x).hypot(polygon[3].y - polygon[0].y) >= 3.0
        {
            result.push(polygon);
        }
    }
    // 先排序行起点，再在相邻近同行文本间插入排序，避免不传递的 sort 比较器。
    result.sort_by(|a, b| a[0].y.total_cmp(&b[0].y).then(a[0].x.total_cmp(&b[0].x)));
    for i in 1..result.len() {
        let mut j = i;
        while j > 0
            && (result[j][0].y - result[j - 1][0].y).abs() < 10.0
            && result[j][0].x < result[j - 1][0].x
        {
            result.swap(j, j - 1);
            j -= 1;
        }
    }
    Ok(result)
}

fn ordered_rectangle(polygon: &Polygon) -> Option<[Coord; 4]> {
    let coordinates = &polygon.exterior().0;
    if coordinates.len() < 4 {
        return None;
    }
    let mut points = [
        coordinates[0],
        coordinates[1],
        coordinates[2],
        coordinates[3],
    ];
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let (left_top, left_bottom) = if points[0].y <= points[1].y {
        (points[0], points[1])
    } else {
        (points[1], points[0])
    };
    let (right_top, right_bottom) = if points[2].y <= points[3].y {
        (points[2], points[3])
    } else {
        (points[3], points[2])
    };
    Some([left_top, right_top, right_bottom, left_bottom])
}

fn score(
    polygon: &Polygon,
    values: &[f32],
    width: usize,
    height: usize,
    operation: &Operation,
) -> Result<f32, Failure> {
    let points = &polygon.exterior().0;
    let min_x = points
        .iter()
        .map(|p| p.x)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .max(0.0) as usize;
    let max_x = (points.iter().map(|p| p.x).fold(0.0, f64::max).ceil() as usize).min(width - 1);
    let min_y = points
        .iter()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min)
        .floor()
        .max(0.0) as usize;
    let max_y = (points.iter().map(|p| p.y).fold(0.0, f64::max).ceil() as usize).min(height - 1);
    let mut sum = 0.0;
    let mut count = 0;
    for y in min_y..=max_y {
        operation.check("db_score")?;
        for x in min_x..=max_x {
            if polygon.contains(&Point::new(x as f64 + 0.5, y as f64 + 0.5)) {
                sum += values[y * width + x];
                count += 1;
            }
        }
    }
    Ok(if count == 0 { 0.0 } else { sum / count as f32 })
}

#[cfg(test)]
#[path = "../../../../../tests/argusflow-vision/unit/ocr/detection/postprocess.rs"]
mod tests;
