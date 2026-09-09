//! 不可变分块快照；只复制真实变化块，按需物化完整图像。

use crate::{PixelRect, PixelView, compare};
use argusflow_core::{EvidenceFrame, EvidencePixelFormat, InspectionFailure, InspectionRect};
use std::sync::Arc;

/// 只读图像块；位置固定在快照的帧本地坐标空间。
#[derive(Clone)]
pub struct PixelBlock {
    bounds: PixelRect,
    pixels: Arc<[u8]>,
}

impl PixelBlock {
    /// 返回块在图像中的位置。
    pub fn bounds(&self) -> PixelRect {
        self.bounds
    }
    /// 返回紧密排列的四通道像素。
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// 固定 32×32 块组成的快照；相邻版本共享未变化的块。
#[derive(Clone)]
pub struct FrameSnapshot {
    bounds: InspectionRect,
    width: u32,
    height: u32,
    format: EvidencePixelFormat,
    blocks: Arc<[PixelBlock]>,
}

impl FrameSnapshot {
    /// 建立首次完整基准。
    pub fn from_frame(frame: &EvidenceFrame) -> Self {
        let mut blocks = Vec::new();
        for y in (0..frame.height()).step_by(32) {
            for x in (0..frame.width()).step_by(32) {
                let bounds = PixelRect {
                    x,
                    y,
                    width: (frame.width() - x).min(32),
                    height: (frame.height() - y).min(32),
                };
                blocks.push(extract(frame, bounds));
            }
        }
        Self {
            bounds: frame.bounds(),
            width: frame.width(),
            height: frame.height(),
            format: frame.format(),
            blocks: blocks.into(),
        }
    }

    /// 精确检测并创建下一份快照，只替换变化块；不同拓扑应由来源建立新基准。
    pub fn update(
        &self,
        frame: &EvidenceFrame,
        candidates: Option<&[PixelRect]>,
    ) -> Result<(Self, Vec<PixelRect>), InspectionFailure> {
        if self.bounds != frame.bounds()
            || self.width != frame.width()
            || self.height != frame.height()
            || self.format != frame.format()
        {
            return Err(InspectionFailure::ContextChanged);
        }
        if let Some(regions) = candidates {
            for rect in regions {
                if rect.width == 0
                    || rect.height == 0
                    || rect
                        .x
                        .checked_add(rect.width)
                        .is_none_or(|right| right > self.width)
                    || rect
                        .y
                        .checked_add(rect.height)
                        .is_none_or(|bottom| bottom > self.height)
                {
                    return Err(InspectionFailure::InvalidGeometry);
                }
            }
        }
        let mut blocks = self.blocks.to_vec();
        let mut changes = Vec::new();
        for block in &mut blocks {
            let rect = block.bounds;
            if candidates.is_some_and(|regions| {
                !regions.iter().any(|candidate| intersects(rect, *candidate))
            }) {
                continue;
            }
            let old = PixelView::new(
                block.pixels(),
                rect.width,
                rect.height,
                rect.width as usize * 4,
                self.format,
            )?;
            let start = (rect.y as usize * self.width as usize + rect.x as usize) * 4;
            let new = PixelView::new(
                &frame.pixels()[start..],
                rect.width,
                rect.height,
                self.width as usize * 4,
                self.format,
            )?;
            let difference = compare(old, new, None)?;
            if difference.changed_pixels() != 0 {
                changes.extend(difference.regions().iter().map(|changed| PixelRect {
                    x: rect.x + changed.x,
                    y: rect.y + changed.y,
                    ..*changed
                }));
                *block = extract(frame, rect);
            }
        }
        Ok((
            Self {
                blocks: blocks.into(),
                ..self.clone()
            },
            changes,
        ))
    }

    /// 返回虚拟屏幕上的物理像素范围。
    pub fn bounds(&self) -> InspectionRect {
        self.bounds
    }

    /// 将原生区域更新应用到共享块，不为局部变化物化完整桌面。
    pub fn apply_patches(
        &self,
        patches: &[EvidenceFrame],
    ) -> Result<(Self, Vec<PixelRect>), InspectionFailure> {
        self.apply(patches, true)
    }

    /// 直接应用原生候选区域，不扫描像素；供录制保留原始呈现，精确比较延后执行。
    pub fn apply_candidates(&self, patches: &[EvidenceFrame]) -> Result<Self, InspectionFailure> {
        self.apply(patches, false).map(|(snapshot, _)| snapshot)
    }

    fn apply(
        &self,
        patches: &[EvidenceFrame],
        precise: bool,
    ) -> Result<(Self, Vec<PixelRect>), InspectionFailure> {
        let mut blocks = self.blocks.to_vec();
        for patch in patches {
            let bounds = patch.bounds();
            if patch.format() != self.format
                || bounds.x < self.bounds.x
                || bounds.y < self.bounds.y
                || bounds.x + bounds.width > self.bounds.x + self.bounds.width
                || bounds.y + bounds.height > self.bounds.y + self.bounds.height
                || bounds.width != f64::from(patch.width())
                || bounds.height != f64::from(patch.height())
                || (bounds.x - self.bounds.x).fract() != 0.0
                || (bounds.y - self.bounds.y).fract() != 0.0
            {
                return Err(InspectionFailure::InvalidGeometry);
            }
        }
        let mut changes = Vec::new();
        for block in &mut blocks {
            let rect = block.bounds;
            let mut pixels: Option<Vec<u8>> = None;
            for patch in patches {
                let region = PixelRect {
                    x: (patch.bounds().x - self.bounds.x) as u32,
                    y: (patch.bounds().y - self.bounds.y) as u32,
                    width: patch.width(),
                    height: patch.height(),
                };
                if !intersects(rect, region) {
                    continue;
                }
                let target = pixels.get_or_insert_with(|| block.pixels.to_vec());
                let left = rect.x.max(region.x);
                let right = (rect.x + rect.width).min(region.x + region.width);
                let top = rect.y.max(region.y);
                let bottom = (rect.y + rect.height).min(region.y + region.height);
                for row in top..bottom {
                    let source = ((row - region.y) as usize * region.width as usize
                        + (left - region.x) as usize)
                        * 4;
                    let destination = ((row - rect.y) as usize * rect.width as usize
                        + (left - rect.x) as usize)
                        * 4;
                    let length = (right - left) as usize * 4;
                    target[destination..destination + length]
                        .copy_from_slice(&patch.pixels()[source..source + length]);
                }
            }
            if let Some(pixels) = pixels {
                if !precise {
                    block.pixels = pixels.into();
                    continue;
                }
                let old = PixelView::new(
                    block.pixels(),
                    rect.width,
                    rect.height,
                    rect.width as usize * 4,
                    self.format,
                )?;
                let new = PixelView::new(
                    &pixels,
                    rect.width,
                    rect.height,
                    rect.width as usize * 4,
                    self.format,
                )?;
                let difference = compare(old, new, None)?;
                if difference.changed_pixels() != 0 {
                    changes.extend(difference.regions().iter().map(|changed| PixelRect {
                        x: rect.x + changed.x,
                        y: rect.y + changed.y,
                        ..*changed
                    }));
                    block.pixels = pixels.into();
                }
            }
        }
        Ok((
            Self {
                blocks: blocks.into(),
                ..self.clone()
            },
            changes,
        ))
    }

    /// 只物化指定帧内区域，用于增量编码与 OCR。
    pub fn crop(&self, region: PixelRect) -> Result<EvidenceFrame, InspectionFailure> {
        if region.width == 0
            || region.height == 0
            || region
                .x
                .checked_add(region.width)
                .is_none_or(|right| right > self.width)
            || region
                .y
                .checked_add(region.height)
                .is_none_or(|bottom| bottom > self.height)
        {
            return Err(InspectionFailure::InvalidGeometry);
        }
        let mut pixels = vec![0; region.width as usize * region.height as usize * 4];
        for block in self
            .blocks
            .iter()
            .filter(|block| intersects(block.bounds, region))
        {
            let rect = block.bounds;
            let left = rect.x.max(region.x);
            let right = (rect.x + rect.width).min(region.x + region.width);
            for row in rect.y.max(region.y)..(rect.y + rect.height).min(region.y + region.height) {
                let source =
                    ((row - rect.y) as usize * rect.width as usize + (left - rect.x) as usize) * 4;
                let destination = ((row - region.y) as usize * region.width as usize
                    + (left - region.x) as usize)
                    * 4;
                let length = (right - left) as usize * 4;
                pixels[destination..destination + length]
                    .copy_from_slice(&block.pixels[source..source + length]);
            }
        }
        EvidenceFrame::new(
            InspectionRect {
                x: self.bounds.x + f64::from(region.x),
                y: self.bounds.y + f64::from(region.y),
                width: region.width.into(),
                height: region.height.into(),
            },
            region.width,
            region.height,
            self.format,
            pixels,
        )
    }
    /// 返回不可变块列表。
    pub fn blocks(&self) -> &[PixelBlock] {
        &self.blocks
    }
    /// 完整图像物化所需的字节数。
    pub fn byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
    /// 按需要重建完整拥有型图像。
    pub fn materialize(&self) -> Result<EvidenceFrame, InspectionFailure> {
        let mut pixels = vec![0; self.byte_len()];
        for block in self.blocks.iter() {
            let rect = block.bounds;
            let row_bytes = rect.width as usize * 4;
            for row in 0..rect.height as usize {
                let start = ((rect.y as usize + row) * self.width as usize + rect.x as usize) * 4;
                pixels[start..start + row_bytes]
                    .copy_from_slice(&block.pixels[row * row_bytes..(row + 1) * row_bytes]);
            }
        }
        EvidenceFrame::new(self.bounds, self.width, self.height, self.format, pixels)
    }
}

fn extract(frame: &EvidenceFrame, bounds: PixelRect) -> PixelBlock {
    let row_bytes = bounds.width as usize * 4;
    let mut pixels = Vec::with_capacity(row_bytes * bounds.height as usize);
    for row in bounds.y..bounds.y + bounds.height {
        let start = (row as usize * frame.width() as usize + bounds.x as usize) * 4;
        pixels.extend_from_slice(&frame.pixels()[start..start + row_bytes]);
    }
    PixelBlock {
        bounds,
        pixels: pixels.into(),
    }
}

fn intersects(a: PixelRect, b: PixelRect) -> bool {
    u64::from(a.x) < u64::from(b.x) + u64::from(b.width)
        && u64::from(b.x) < u64::from(a.x) + u64::from(a.width)
        && u64::from(a.y) < u64::from(b.y) + u64::from(b.height)
        && u64::from(b.y) < u64::from(a.y) + u64::from(a.height)
}
