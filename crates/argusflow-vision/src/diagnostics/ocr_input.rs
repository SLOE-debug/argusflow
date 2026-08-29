//! 将最近一次失败的 OCR 输入保存为无需额外图片依赖即可查看的 BMP。

use crate::{frame::PixelFormat, image::PixelImage};

/// 把 OCR 使用的 BGRA 图片原样编码为常见查看器可直接打开的 32 位 top-down BMP。
pub fn encode_bgra_as_bmp(image: &PixelImage) -> Result<Vec<u8>, String> {
    if image.format != PixelFormat::Bgra8Unorm {
        return Err(format!(
            "unsupported diagnostic pixel format: {:?}",
            image.format
        ));
    }
    let width = i32::try_from(image.width).map_err(|_| "image width exceeds BMP limits")?;
    let height = i32::try_from(image.height).map_err(|_| "image height exceeds BMP limits")?;
    let source_row_bytes = image.width as usize * image.format.bytes_per_pixel();
    let pixel_bytes = source_row_bytes
        .checked_mul(image.height as usize)
        .ok_or("BMP pixel byte length overflow")?;
    let file_bytes = 54_usize
        .checked_add(pixel_bytes)
        .ok_or("BMP file byte length overflow")?;
    let file_size = u32::try_from(file_bytes).map_err(|_| "BMP file exceeds 4 GiB")?;
    let pixel_size = u32::try_from(pixel_bytes).map_err(|_| "BMP pixels exceed 4 GiB")?;

    let mut output = Vec::with_capacity(file_bytes);
    output.extend_from_slice(b"BM");
    output.extend_from_slice(&file_size.to_le_bytes());
    output.extend_from_slice(&[0_u8; 4]);
    output.extend_from_slice(&54_u32.to_le_bytes());
    output.extend_from_slice(&40_u32.to_le_bytes());
    output.extend_from_slice(&width.to_le_bytes());
    // 负高度使 BMP 按 OCR 输入相同的从上到下行序保存，避免额外翻转。
    output.extend_from_slice(&(-height).to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&32_u16.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&pixel_size.to_le_bytes());
    output.extend_from_slice(&[0_u8; 16]);

    for row in 0..image.height as usize {
        let row_start = row * image.stride_bytes;
        let source_row = image
            .pixels()
            .get(row_start..row_start + source_row_bytes)
            .ok_or("pixel image row is shorter than its declared stride")?;
        output.extend_from_slice(source_row);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_encoder_preserves_top_down_bgra_pixels() {
        let image = PixelImage::new(
            2,
            1,
            8,
            PixelFormat::Bgra8Unorm,
            vec![1, 2, 3, 0, 4, 5, 6, 255],
        )
        .expect("fixture is a valid BGRA image");

        let bitmap = encode_bgra_as_bmp(&image).expect("diagnostic BMP should encode");

        assert_eq!(&bitmap[..2], b"BM");
        assert_eq!(u32::from_le_bytes(bitmap[2..6].try_into().unwrap()), 62);
        assert_eq!(i32::from_le_bytes(bitmap[22..26].try_into().unwrap()), -1);
        assert_eq!(&bitmap[54..62], &[1, 2, 3, 0, 4, 5, 6, 255]);
    }
}
