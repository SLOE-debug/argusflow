//! 直接对NV12分块求均值，保留亮度及色度；不生成全分辨率RGB中间图。
use super::model::{Result, VideoError};
#[cfg(test)]
#[path = "../../../../../tests/argusflow-windows/unit/capture/video_thumbnail.rs"]
mod tests;
/// 用于变化分析的有界YCbCr缩略图，不作为展示证据替代原帧。
pub struct VideoThumbnail {
    /// 缩略图宽度，最多640。
    pub width: u32,
    /// 缩略图高度，最多400。
    pub height: u32,
    /// 每个像素依次为Y/Cb/Cr。
    pub pixels: Vec<[u8; 3]>,
}
pub(super) fn thumbnail(
    bytes: &[u8],
    width: u32,
    height: u32,
    buffer_height: u32,
    stride: i32,
) -> Result<VideoThumbnail> {
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
        return Err(VideoError::Invalid("视频缩略图缓冲无效".into()));
    }
    let step = width
        .div_ceil(640)
        .max(height.div_ceil(400))
        .max(2)
        .div_ceil(2)
        * 2;
    let columns = width.div_ceil(step);
    let rows = height.div_ceil(step);
    let mut pixels = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows {
        for col in 0..columns {
            let (x0, y0) = (col * step, row * step);
            let (x1, y1) = ((x0 + step).min(width), (y0 + step).min(height));
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    sum[0] += u32::from(bytes[y as usize * pitch + x as usize]);
                }
            }
            for y in (y0..y1).step_by(2) {
                for x in (x0..x1).step_by(2) {
                    let uv = pitch * buffer_height as usize + y as usize / 2 * pitch + x as usize;
                    sum[1] += u32::from(bytes[uv]);
                    sum[2] += u32::from(bytes[uv + 1]);
                }
            }
            let count = (x1 - x0) * (y1 - y0);
            pixels.push([
                (sum[0] / count) as u8,
                (sum[1] / (count / 4)) as u8,
                (sum[2] / (count / 4)) as u8,
            ]);
        }
    }
    Ok(VideoThumbnail {
        width: columns,
        height: rows,
        pixels,
    })
}
