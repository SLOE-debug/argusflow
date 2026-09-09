//! 驱动的 dirty/move 元数据只作为精确比较候选。
use crate::capture::gpu::{failure, invalid};
use argusflow_capture_contracts::{CaptureResult, PixelRect};
use windows::Win32::{
    Foundation::RECT,
    Graphics::Dxgi::{DXGI_OUTDUPL_MOVE_RECT, IDXGIOutputDuplication},
};
pub(super) fn read(
    duplication: &IDXGIOutputDuplication,
    bytes: u32,
    width: u32,
    height: u32,
) -> CaptureResult<Vec<PixelRect>> {
    if bytes == 0 {
        return Ok(vec![PixelRect::new(0, 0, width, height)?]);
    }
    if bytes > 1024 * 1024 {
        return Err(invalid("DXGI metadata exceeds budget"));
    }
    let mut dirty = vec![RECT::default(); (bytes as usize).div_ceil(std::mem::size_of::<RECT>())];
    let mut required = 0;
    // SAFETY: 分配覆盖驱动声明的整个元数据区，调用方持有帧租约。
    unsafe { duplication.GetFrameDirtyRects(bytes, dirty.as_mut_ptr(), &mut required) }
        .map_err(failure)?;
    if required > bytes || !(required as usize).is_multiple_of(std::mem::size_of::<RECT>()) {
        return Err(invalid("dirty metadata length"));
    }
    dirty.truncate(required as usize / std::mem::size_of::<RECT>());
    let mut moves = vec![
        DXGI_OUTDUPL_MOVE_RECT::default();
        (bytes as usize).div_ceil(std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>())
    ];
    unsafe { duplication.GetFrameMoveRects(bytes, moves.as_mut_ptr(), &mut required) }
        .map_err(failure)?;
    if required > bytes
        || !(required as usize).is_multiple_of(std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>())
    {
        return Err(invalid("move metadata length"));
    }
    moves.truncate(required as usize / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>());
    dirty.extend(moves.into_iter().map(|region| region.DestinationRect));
    if dirty.len() > 8192 {
        return Err(invalid("too many dirty rectangles"));
    }
    let bounds = PixelRect::new(0, 0, width, height)?;
    dirty
        .into_iter()
        .map(|rect| {
            if rect.left < 0 || rect.top < 0 || rect.right <= rect.left || rect.bottom <= rect.top {
                return Err(invalid("dirty rectangle geometry"));
            }
            let rect = PixelRect::new(
                rect.left as u32,
                rect.top as u32,
                (rect.right - rect.left) as u32,
                (rect.bottom - rect.top) as u32,
            )?;
            if !bounds.contains(rect) {
                return Err(invalid("dirty rectangle outside output"));
            }
            Ok(rect)
        })
        .collect()
}
