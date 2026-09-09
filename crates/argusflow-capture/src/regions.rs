//! 按共享边合并收紧后的矩形，不将分离变化扩大为整屏。
use crate::PixelRect;

/// 合并具有相同跨度的相邻块，保证不引入额外空白面积。
pub fn merge_regions(mut regions: Vec<PixelRect>) -> Vec<PixelRect> {
    regions.sort_unstable_by_key(|rect| (rect.y, rect.height, rect.x));
    let mut horizontal: Vec<PixelRect> = Vec::with_capacity(regions.len());
    for region in regions {
        if let Some(previous) = horizontal.last_mut() {
            if previous.y == region.y
                && previous.height == region.height
                && previous.x + previous.width == region.x
            {
                previous.width += region.width;
                continue;
            }
        }
        horizontal.push(region);
    }
    horizontal.sort_unstable_by_key(|rect| (rect.x, rect.width, rect.y));
    let mut merged: Vec<PixelRect> = Vec::with_capacity(horizontal.len());
    for region in horizontal {
        if let Some(previous) = merged.last_mut() {
            if previous.x == region.x
                && previous.width == region.width
                && previous.y + previous.height == region.y
            {
                previous.height += region.height;
                continue;
            }
        }
        merged.push(region);
    }
    merged.sort_unstable_by_key(|rect| (rect.y, rect.x));
    merged
}
