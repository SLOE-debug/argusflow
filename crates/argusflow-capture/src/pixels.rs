//! 原始分辨率区域的有界复制与精确比较。
use argusflow_capture_contracts::*;
use argusflow_core::{FailureKind, Operation};
use argusflow_image::{ImageView, has_changes};

pub(crate) fn differs(
    a: &PixelImage,
    b: &PixelImage,
    region: PixelRect,
    operation: &Operation,
) -> CaptureResult<bool> {
    has_changes(
        ImageView::region(a, region)?,
        ImageView::region(b, region)?,
        1,
        operation,
    )
}

/// 将上次区域快照直接与当前完整帧的对应视图比较，避免先复制再判断。
pub(crate) fn region_changed(
    previous: &PixelImage,
    current: &PixelImage,
    region: PixelRect,
    operation: &Operation,
) -> CaptureResult<bool> {
    has_changes(
        ImageView::region(
            previous,
            PixelRect::new(0, 0, previous.width(), previous.height())?,
        )?,
        ImageView::region(current, region)?,
        1,
        operation,
    )
}

pub(crate) fn crop(
    image: &PixelImage,
    region: PixelRect,
    budget: &ByteBudget,
    operation: &Operation,
) -> CaptureResult<PixelImage> {
    ImageView::region(image, region)?;
    let size = usize::try_from(region.byte_len()).map_err(|_| {
        CaptureError::new(FailureKind::ResourceLimit, "region_copy", "区域字节数溢出")
    })?;
    let reservation = budget.reserve(size)?;
    let stride = region.width() as usize * 4;
    let mut bytes = vec![0; size];
    for (row, target) in bytes.chunks_exact_mut(stride).enumerate() {
        operation.check("region_copy")?;
        let start = (region.y() as usize + row) * image.stride() + region.x() as usize * 4;
        target.copy_from_slice(&image.bytes()[start..start + stride]);
    }
    PixelImage::new(
        region.width(),
        region.height(),
        stride,
        image.format(),
        bytes,
        reservation,
    )
}
