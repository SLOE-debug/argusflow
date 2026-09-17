//! 录制契约BT.709 limited-range NV12转RGBA，只转换最终选中的帧。
use super::model::{Result, VideoError};
pub(super) fn rgba(
    bytes: &[u8],
    width: u32,
    height: u32,
    buffer_height: u32,
    stride: i32,
) -> Result<Vec<u8>> {
    let pitch = stride.unsigned_abs() as usize;
    if stride <= 0
        || width == 0
        || height == 0
        || !width.is_multiple_of(2)
        || !height.is_multiple_of(2)
        || buffer_height < height
        || !buffer_height.is_multiple_of(2)
        || pitch < width as usize
        || pitch * buffer_height as usize * 3 / 2 > bytes.len()
        || u64::from(width) * u64::from(height) > 3840 * 2160
    {
        return Err(VideoError::Invalid("视频像素步长或长度错误".into()));
    }
    let mut rgba = vec![0; width as usize * height as usize * 4];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let uv = pitch * buffer_height as usize + (y / 2) * pitch + (x & !1);
            let c = i32::from(bytes[y * pitch + x]) - 16;
            let d = i32::from(bytes[uv]) - 128;
            let e = i32::from(bytes[uv + 1]) - 128;
            let dst = (y * width as usize + x) * 4;
            rgba[dst..dst + 4].copy_from_slice(&[
                ((298 * c + 459 * e + 128) >> 8).clamp(0, 255) as u8,
                ((298 * c - 55 * d - 136 * e + 128) >> 8).clamp(0, 255) as u8,
                ((298 * c + 541 * d + 128) >> 8).clamp(0, 255) as u8,
                255,
            ]);
        }
    }
    Ok(rgba)
}
#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/video_pixels.rs"]
mod tests;
