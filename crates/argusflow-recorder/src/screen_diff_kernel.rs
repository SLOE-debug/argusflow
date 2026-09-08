//! 无分配的分块差异核；与采集、事件和 PNG 完全独立。

/// 返回变化的 32×32 块数，逐块提前结束；调用方必须提供紧密排列的四通道像素。
#[allow(dead_code)] // 独立微基准和测试使用；生产调用同一个 scan 内核并收集变化位置。
pub(crate) fn count(
    previous: &[u8],
    current: &[u8],
    width: usize,
    height: usize,
    vector: bool,
) -> usize {
    scan(previous, current, width, height, vector, |_, _, _, _| {})
}

/// 一次比较同时输出变化块，避免裁剪阶段再次扫描整帧。
pub(crate) fn scan(
    previous: &[u8],
    current: &[u8],
    width: usize,
    height: usize,
    vector: bool,
    mut changed_block: impl FnMut(usize, usize, usize, usize),
) -> usize {
    assert_eq!(previous.len(), width * height * 4);
    assert_eq!(current.len(), previous.len());
    let mut changed = 0;
    for top in (0..height).step_by(32) {
        for left in (0..width).step_by(32) {
            let bytes = (width - left).min(32) * 4;
            if (top..(top + 32).min(height)).any(|row| {
                let start = (row * width + left) * 4;
                let a = &previous[start..start + bytes];
                let b = &current[start..start + bytes];
                if vector { row_changed(a, b) } else { a != b }
            }) {
                changed += 1;
                changed_block(left, top, (width - left).min(32), (height - top).min(32));
            }
        }
    }
    changed
}

/// x86-64 基线 SSE2，不要求低端 CPU 提供 AVX2；短尾部使用精确字节比较。
fn row_changed(a: &[u8], b: &[u8]) -> bool {
    #[cfg(target_arch = "x86_64")]
    if a.len() == 128 {
        use std::arch::x86_64::*;
        // SAFETY: 两个切片都是 128 字节，非对齐加载每次 16 字节且从不越界。
        // SSE2 是 x86-64 的基线指令集，结果只检测字节相等，不依赖通道语义。
        unsafe {
            let mut different = _mm_setzero_si128();
            for offset in (0..128).step_by(16) {
                let left = _mm_loadu_si128(a.as_ptr().add(offset).cast());
                let right = _mm_loadu_si128(b.as_ptr().add(offset).cast());
                different = _mm_or_si128(different, _mm_xor_si128(left, right));
            }
            return _mm_movemask_epi8(_mm_cmpeq_epi8(different, _mm_setzero_si128())) != 0xffff;
        }
    }
    a != b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_matches_scalar_for_every_byte_and_partial_rows() {
        for length in [4, 28, 124, 128] {
            let original = vec![42; length];
            assert!(!row_changed(&original, &original));
            for position in 0..length {
                let mut changed = original.clone();
                changed[position] ^= 1;
                assert!(row_changed(&original, &changed));
            }
        }
    }
}
