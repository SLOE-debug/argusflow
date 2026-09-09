//! 对候选区域进行精确颜色比较；不通过降采样忽略细笔画。

use argusflow_core::{EvidencePixelFormat, InspectionFailure};

/// 帧本地的非空整数像素区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    /// 左边界。
    pub x: u32,
    /// 上边界。
    pub y: u32,
    /// 水平像素数。
    pub width: u32,
    /// 垂直像素数。
    pub height: u32,
}

/// 已校验的只读像素视图，不获取或复制底层内存所有权。
#[derive(Clone, Copy)]
pub struct PixelView<'a> {
    pixels: &'a [u8],
    width: u32,
    height: u32,
    stride: usize,
    format: EvidencePixelFormat,
}

impl<'a> PixelView<'a> {
    /// 验证布局与缓冲长度；支持带行尾填充的四通道图像。
    pub fn new(
        pixels: &'a [u8],
        width: u32,
        height: u32,
        stride: usize,
        format: EvidencePixelFormat,
    ) -> Result<Self, InspectionFailure> {
        let row = (width as usize)
            .checked_mul(4)
            .ok_or(InspectionFailure::InvalidGeometry)?;
        let length = stride
            .checked_mul(height.saturating_sub(1) as usize)
            .and_then(|length| length.checked_add(row))
            .ok_or(InspectionFailure::InvalidGeometry)?;
        if width == 0 || height == 0 || stride < row || pixels.len() < length {
            return Err(InspectionFailure::InvalidGeometry);
        }
        Ok(Self {
            pixels,
            width,
            height,
            stride,
            format,
        })
    }
}

/// 一次比较的精确计数和互不重叠的变化矩形。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    regions: Vec<PixelRect>,
    compared_pixels: u64,
    changed_pixels: u64,
}

impl ChangeSet {
    /// 返回收紧后的变化区域，不暴露可变集合。
    pub fn regions(&self) -> &[PixelRect] {
        &self.regions
    }
    /// 实际比较的像素数量，不包含重复候选区域。
    pub fn compared_pixels(&self) -> u64 {
        self.compared_pixels
    }
    /// 有效颜色通道发生变化的像素数。
    pub fn changed_pixels(&self) -> u64 {
        self.changed_pixels
    }
}

/// 在候选覆盖的 32×32 块内比较；None 表示全帧，空候选表示无需比较。
pub fn compare(
    previous: PixelView<'_>,
    current: PixelView<'_>,
    candidates: Option<&[PixelRect]>,
) -> Result<ChangeSet, InspectionFailure> {
    if previous.width != current.width
        || previous.height != current.height
        || previous.format != current.format
    {
        return Err(InspectionFailure::InvalidGeometry);
    }
    let columns = current.width.div_ceil(32) as usize;
    let rows = current.height.div_ceil(32) as usize;
    let mut selected = vec![candidates.is_none(); columns * rows];
    if let Some(candidates) = candidates {
        for rect in candidates {
            let right = rect
                .x
                .checked_add(rect.width)
                .ok_or(InspectionFailure::InvalidGeometry)?;
            let bottom = rect
                .y
                .checked_add(rect.height)
                .ok_or(InspectionFailure::InvalidGeometry)?;
            if rect.width == 0
                || rect.height == 0
                || right > current.width
                || bottom > current.height
            {
                return Err(InspectionFailure::InvalidGeometry);
            }
            for y in rect.y / 32..bottom.div_ceil(32) {
                for x in rect.x / 32..right.div_ceil(32) {
                    selected[y as usize * columns + x as usize] = true;
                }
            }
        }
    }
    let mut result = ChangeSet {
        regions: Vec::new(),
        compared_pixels: 0,
        changed_pixels: 0,
    };
    for (tile, selected) in selected.into_iter().enumerate() {
        if !selected {
            continue;
        }
        let left = (tile % columns) as u32 * 32;
        let top = (tile / columns) as u32 * 32;
        let right = (left + 32).min(current.width);
        let bottom = (top + 32).min(current.height);
        let mut bounds = (right, bottom, left, top);
        for y in top..bottom {
            result.compared_pixels += u64::from(right - left);
            let old_row = y as usize * previous.stride + left as usize * 4;
            let new_row = y as usize * current.stride + left as usize * 4;
            let row_bytes = (right - left) as usize * 4;
            if crate::kernel::equal(
                &previous.pixels[old_row..old_row + row_bytes],
                &current.pixels[new_row..new_row + row_bytes],
                current.format,
            ) {
                continue;
            }
            for x in left..right {
                let old = y as usize * previous.stride + x as usize * 4;
                let new = y as usize * current.stride + x as usize * 4;
                let channels = match current.format {
                    EvidencePixelFormat::Bgrx8 => 3,
                    EvidencePixelFormat::Rgba8 => 4,
                };
                if previous.pixels[old..old + channels] != current.pixels[new..new + channels] {
                    result.changed_pixels += 1;
                    bounds.0 = bounds.0.min(x);
                    bounds.1 = bounds.1.min(y);
                    bounds.2 = bounds.2.max(x + 1);
                    bounds.3 = bounds.3.max(y + 1);
                }
            }
        }
        if bounds.0 < bounds.2 && bounds.1 < bounds.3 {
            result.regions.push(PixelRect {
                x: bounds.0,
                y: bounds.1,
                width: bounds.2 - bounds.0,
                height: bounds.3 - bounds.1,
            });
        }
    }
    result.regions = crate::merge_regions(result.regions);
    Ok(result)
}
