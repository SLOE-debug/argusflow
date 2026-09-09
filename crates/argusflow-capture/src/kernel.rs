//! 四通道像素行的精确 SIMD 快速排除；BGRX 的第四字节不参与比较。

use argusflow_core::EvidencePixelFormat;

pub(crate) fn equal(a: &[u8], b: &[u8], format: EvidencePixelFormat) -> bool {
    let ignore_reserved = format == EvidencePixelFormat::Bgrx8;
    let mut offset = 0;
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::*;
        // SAFETY: x86-64 保证 SSE2，循环每次只加载两个切片中完整的 16 字节。
        unsafe {
            let mask = _mm_set1_epi32(if ignore_reserved { 0x00ffffff } else { -1 });
            while offset + 16 <= a.len() {
                let left = _mm_loadu_si128(a.as_ptr().add(offset).cast());
                let right = _mm_loadu_si128(b.as_ptr().add(offset).cast());
                let difference = _mm_and_si128(_mm_xor_si128(left, right), mask);
                if _mm_movemask_epi8(_mm_cmpeq_epi8(difference, _mm_setzero_si128())) != 0xffff {
                    return false;
                }
                offset += 16;
            }
        }
    }
    let channels = if ignore_reserved { 3 } else { 4 };
    a[offset..]
        .chunks_exact(4)
        .zip(b[offset..].chunks_exact(4))
        .all(|(left, right)| left[..channels] == right[..channels])
}
