//! 分块精确差异：无颜色转换、无叠图分配，单块发现差异后立即跳过剩余行。
use argusflow_core::EvidenceFrame;

/// 32×32 像素分块；返回变化块数，零表示整个桌面像素相同。
pub(crate) fn changed_tiles(previous: &EvidenceFrame, current: &EvidenceFrame) -> usize {
    let width = current.width() as usize;
    let height = current.height() as usize;
    if previous.bounds() != current.bounds()
        || previous.format() != current.format()
        || previous.width() != current.width()
        || previous.height() != current.height()
    {
        return width.div_ceil(32) * height.div_ceil(32);
    }
    crate::screen_diff_kernel::count(previous.pixels(), current.pixels(), width, height, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_changes_and_partial_edge_tiles_are_counted_once() {
        let bounds = argusflow_core::InspectionRect {
            x: -10.0,
            y: 0.0,
            width: 65.0,
            height: 33.0,
        };
        let original = EvidenceFrame::new(
            bounds,
            65,
            33,
            argusflow_core::EvidencePixelFormat::Rgba8,
            vec![0; 65 * 33 * 4],
        )
        .unwrap();
        assert_eq!(changed_tiles(&original, &original), 0);
        let mut pixels = original.pixels().to_vec();
        pixels[0] = 255;
        pixels[4] = 255;
        pixels[(65 * 33 - 1) * 4] = 255;
        let changed = EvidenceFrame::new(bounds, 65, 33, original.format(), pixels).unwrap();
        assert_eq!(changed_tiles(&original, &changed), 2);
    }
}
