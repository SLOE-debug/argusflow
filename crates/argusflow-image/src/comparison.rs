//! 按行分块跳过相同字节，仅对不同块执行颜色归一化比较。
use crate::ImageView;
use argusflow_capture_contracts::CaptureResult;
use argusflow_capture_contracts::PixelFormat;
use argusflow_core::Operation;
use std::ops::ControlFlow;

pub(crate) fn visit_changes(
    a: ImageView<'_>,
    b: ImageView<'_>,
    threshold: u8,
    operation: &Operation,
    mut changed: impl FnMut(usize) -> ControlFlow<()>,
) -> CaptureResult<bool> {
    operation.check("image_compare")?;
    let same_layout = a.format == b.format && a.channels == b.channels;
    if same_layout && std::ptr::eq(a.bytes, b.bytes) && a.stride == b.stride && a.region == b.region
    {
        return Ok(false);
    }
    let width = a.width() as usize;
    for y in 0..a.height() as usize {
        for x in (0..width).step_by(256) {
            operation.check("image_compare")?;
            let end = (x + 256).min(width);
            if same_layout && a.row_chunk(y, x, end) == b.row_chunk(y, x, end) {
                continue;
            }
            // 同格式无透明度时，通道排列不影响差值；直接比较，省掉逐像素坐标除法和白底合成。
            if same_layout && matches!(a.format, None | Some(PixelFormat::Bgrx8)) {
                for (offset, (left, right)) in a
                    .row_chunk(y, x, end)
                    .chunks_exact(a.channels)
                    .zip(b.row_chunk(y, x, end).chunks_exact(b.channels))
                    .enumerate()
                {
                    let differs = if threshold == 1 {
                        left[..3] != right[..3]
                    } else {
                        (0..3).any(|channel| left[channel].abs_diff(right[channel]) >= threshold)
                    };
                    if differs && changed(y * width + x + offset).is_break() {
                        return Ok(true);
                    }
                }
                continue;
            }
            for i in y * width + x..y * width + end {
                if a.pixel(i)
                    .into_iter()
                    .zip(b.pixel(i))
                    .any(|(a, b)| a.abs_diff(b) >= threshold)
                    && changed(i).is_break()
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
