//! DXGI 租约内读取系统变化候选；移动目标使用当前帧的最终像素。

use argusflow_core::InspectionFailure;
use windows::Win32::{
    Foundation::RECT,
    Graphics::Dxgi::{DXGI_OUTDUPL_MOVE_RECT, IDXGIOutputDuplication},
};

/// 只在成功取得且尚未释放的帧租约内调用。
pub(super) fn regions(
    duplication: &IDXGIOutputDuplication,
    bytes: u32,
) -> Result<Vec<RECT>, InspectionFailure> {
    if bytes == 0 {
        return Ok(Vec::new());
    }
    // DXGI 元数据量也受界限约束，避免错误驱动导致无界分配。
    if bytes > 16 * 1024 * 1024 {
        return Err(InspectionFailure::InvalidGeometry);
    }
    let mut dirty = vec![RECT::default(); (bytes as usize).div_ceil(std::mem::size_of::<RECT>())];
    let mut required = 0;
    // SAFETY: 向量长度覆盖 bytes，输出大小独占，调用方持有有效租约。
    unsafe { duplication.GetFrameDirtyRects(bytes, dirty.as_mut_ptr(), &mut required) }
        .map_err(|_| InspectionFailure::Unavailable)?;
    if required > bytes || required as usize % std::mem::size_of::<RECT>() != 0 {
        return Err(InspectionFailure::InvalidGeometry);
    }
    dirty.truncate(required as usize / std::mem::size_of::<RECT>());
    let mut moves = vec![
        DXGI_OUTDUPL_MOVE_RECT::default();
        (bytes as usize).div_ceil(std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>())
    ];
    unsafe { duplication.GetFrameMoveRects(bytes, moves.as_mut_ptr(), &mut required) }
        .map_err(|_| InspectionFailure::Unavailable)?;
    if required > bytes || required as usize % std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>() != 0 {
        return Err(InspectionFailure::InvalidGeometry);
    }
    moves.truncate(required as usize / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>());
    dirty.extend(moves.into_iter().map(|movement| movement.DestinationRect));
    Ok(dirty)
}
