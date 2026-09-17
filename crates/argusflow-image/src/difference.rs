//! 像素阈值差分与四连通分量；过滤规则由消费者显式提供。
use crate::{ImageView, view::invalid};
use argusflow_capture_contracts::{CaptureError, CaptureResult, PixelRect};
use argusflow_core::{FailureKind, Operation};

/// 连通分量的包围框与实际变化像素数，坐标相对于输入视图。
#[derive(Debug, Clone, Copy)]
pub struct ChangeRegion {
    /// 包围框可以包含内部未变像素。
    pub bounds: PixelRect,
    /// 实际发生变化的像素数。
    pub pixels: usize,
}
/// 连通分量过滤；默认精确比较，不丢弃细线或单像素。
#[derive(Debug, Clone, Copy)]
pub struct DifferencePolicy {
    /// 任一通道差值达到此值即变化，必须非零。
    pub threshold: u8,
    /// 分量至少包含的变化像素数。
    pub min_pixels: usize,
    /// 分量包围框最小宽度。
    pub min_width: u32,
    /// 分量包围框最小高度。
    pub min_height: u32,
}
impl Default for DifferencePolicy {
    fn default() -> Self {
        Self {
            threshold: 1,
            min_pixels: 1,
            min_width: 1,
            min_height: 1,
        }
    }
}
fn validate(a: ImageView<'_>, b: ImageView<'_>, threshold: u8) -> CaptureResult<usize> {
    if threshold == 0 || a.width() != b.width() || a.height() != b.height() {
        return Err(invalid("比较尺寸不一致或阈值为零"));
    }
    Ok(a.width() as usize * a.height() as usize)
}
fn differs(a: ImageView<'_>, b: ImageView<'_>, i: usize, threshold: u8) -> bool {
    a.pixel(i)
        .into_iter()
        .zip(b.pixel(i))
        .any(|(a, b)| a.abs_diff(b) >= threshold)
}
/// 只判断是否变化，精确模式阈值为 1；不分配连通分量缓冲。
pub fn has_changes(
    a: ImageView<'_>,
    b: ImageView<'_>,
    threshold: u8,
    operation: &Operation,
) -> CaptureResult<bool> {
    let count = validate(a, b, threshold)?;
    for i in 0..count {
        if i % 4096 == 0 {
            operation.check("image_compare")?;
        }
        if differs(a, b, i, threshold) {
            return Ok(true);
        }
    }
    Ok(false)
}
/// 提取四连通变化区域，按面积降序返回；最多 65536 个有效分量，超限明确失败。
pub fn changed_regions(
    a: ImageView<'_>,
    b: ImageView<'_>,
    policy: DifferencePolicy,
    operation: &Operation,
) -> CaptureResult<Vec<ChangeRegion>> {
    let count = validate(a, b, policy.threshold)?;
    if policy.min_pixels == 0 || policy.min_width == 0 || policy.min_height == 0 {
        return Err(invalid("分量过滤尺寸必须非零"));
    }
    let mut mask = vec![false; count];
    for (i, changed) in mask.iter_mut().enumerate() {
        if i % 4096 == 0 {
            operation.check("image_difference")?;
        }
        *changed = differs(a, b, i, policy.threshold);
    }
    let width = a.width() as usize;
    let height = a.height() as usize;
    let mut regions = Vec::new();
    let mut pending = Vec::<u32>::new();
    for start in 0..count {
        if start % 4096 == 0 {
            operation.check("image_components")?;
        }
        if !mask[start] {
            continue;
        }
        mask[start] = false;
        pending.push(start as u32);
        let (mut left, mut top, mut right, mut bottom, mut area) = (width, height, 0, 0, 0);
        while let Some(i) = pending.pop() {
            let i = i as usize;
            if area % 4096 == 0 {
                operation.check("image_component")?;
            }
            let (x, y) = (i % width, i / width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
            area += 1;
            for neighbor in [
                x.checked_sub(1).map(|_| i - 1),
                (x + 1 < width).then_some(i + 1),
                y.checked_sub(1).map(|_| i - width),
                (y + 1 < height).then_some(i + width),
            ]
            .into_iter()
            .flatten()
            {
                if mask[neighbor] {
                    mask[neighbor] = false;
                    pending.push(neighbor as u32);
                }
            }
        }
        let bounds = PixelRect::new(
            left as u32,
            top as u32,
            (right - left + 1) as u32,
            (bottom - top + 1) as u32,
        )?;
        if area >= policy.min_pixels
            && bounds.width() >= policy.min_width
            && bounds.height() >= policy.min_height
        {
            if regions.len() == 65_536 {
                return Err(CaptureError::new(
                    FailureKind::ResourceLimit,
                    "image_components",
                    "变化区域数量超限",
                ));
            }
            regions.push(ChangeRegion {
                bounds,
                pixels: area,
            });
        }
    }
    regions.sort_by_key(|region| std::cmp::Reverse(region.pixels));
    Ok(regions)
}

#[cfg(test)]
#[path = "../../../tests/argusflow-image/unit/difference.rs"]
mod tests;
