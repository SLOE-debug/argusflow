//! 精确覆盖的矩形集合运算；不以大包围盒合并远离的变化。
use argusflow_capture_contracts::{CaptureError, CaptureResult, PixelRect};
use argusflow_core::FailureKind;
use std::collections::BTreeMap;

const MAX_REGIONS: usize = 8192;
fn bounded(length: usize) -> CaptureResult<()> {
    if length > MAX_REGIONS {
        Err(CaptureError::new(
            FailureKind::ResourceLimit,
            "regions",
            "区域数量超过预算",
        ))
    } else {
        Ok(())
    }
}
/// 求无重叠的矩形并集，按水平带合并，不漏像素也不任意扩大区域。
pub fn normalize_regions(regions: &[PixelRect]) -> CaptureResult<Vec<PixelRect>> {
    bounded(regions.len())?;
    let mut ys: Vec<_> = regions.iter().flat_map(|r| [r.y(), r.bottom()]).collect();
    ys.sort_unstable();
    ys.dedup();
    let mut output: Vec<PixelRect> = Vec::new();
    let mut previous = BTreeMap::<(u32, u32), usize>::new();
    for band in ys.windows(2) {
        let mut spans: Vec<_> = regions
            .iter()
            .filter(|r| r.y() <= band[0] && r.bottom() >= band[1])
            .map(|r| (r.x(), r.right()))
            .collect();
        spans.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::new();
        for (left, right) in spans {
            if let Some(last) = merged.last_mut()
                && left <= last.1
            {
                last.1 = last.1.max(right);
                continue;
            }
            merged.push((left, right));
        }
        let mut current = BTreeMap::new();
        for (left, right) in merged {
            let key = (left, right);
            let index = if let Some(&index) = previous.get(&key) {
                let old = output[index];
                output[index] = PixelRect::new(left, old.y(), right - left, band[1] - old.y())?;
                index
            } else {
                output.push(PixelRect::new(
                    left,
                    band[0],
                    right - left,
                    band[1] - band[0],
                )?);
                output.len() - 1
            };
            current.insert(key, index);
        }
        bounded(output.len())?;
        previous = current;
    }
    output.sort_unstable();
    Ok(output)
}
/// 从区域集合减去显式忽略区，返回无重叠覆盖。
pub fn subtract_regions(
    regions: &[PixelRect],
    ignored: &[PixelRect],
) -> CaptureResult<Vec<PixelRect>> {
    bounded(ignored.len())?;
    let mut pieces = normalize_regions(regions)?;
    for mask in ignored {
        let mut next = Vec::new();
        for region in pieces {
            let Some(cut) = region.intersection(*mask) else {
                next.push(region);
                continue;
            };
            if region.y() < cut.y() {
                next.push(PixelRect::new(
                    region.x(),
                    region.y(),
                    region.width(),
                    cut.y() - region.y(),
                )?);
            }
            if cut.bottom() < region.bottom() {
                next.push(PixelRect::new(
                    region.x(),
                    cut.bottom(),
                    region.width(),
                    region.bottom() - cut.bottom(),
                )?);
            }
            if region.x() < cut.x() {
                next.push(PixelRect::new(
                    region.x(),
                    cut.y(),
                    cut.x() - region.x(),
                    cut.height(),
                )?);
            }
            if cut.right() < region.right() {
                next.push(PixelRect::new(
                    cut.right(),
                    cut.y(),
                    region.right() - cut.right(),
                    cut.height(),
                )?);
            }
        }
        bounded(next.len())?;
        pieces = next;
    }
    normalize_regions(&pieces)
}
/// 扩边、裁边、去重，精确变化范围由调用方另行保存。
pub fn reading_regions(
    regions: &[PixelRect],
    padding: u32,
    bounds: PixelRect,
) -> CaptureResult<Vec<PixelRect>> {
    normalize_regions(
        &regions
            .iter()
            .filter_map(|r| r.expanded(padding, bounds))
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
#[path = "../../../tests/argusflow-capture/unit/regions.rs"]
mod tests;
