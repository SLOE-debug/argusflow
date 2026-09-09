//! 对已保存的 PNG 执行本地 OCR，不触发实时截图或执行定位。
use crate::{
    FrameId, OcrEngine, OcrItem, OcrProfile, OcrRequest, OcrRequestId, PhysicalRect, PixelFormat,
    PixelImage, TopologyGeneration, VisionError,
};
use argusflow_core::WindowIdentity;
use std::{io::Cursor, time::Duration};

/// 解码已有 RGBA PNG 并交给同一个 Paddle worker，输出原图坐标文字框。
pub async fn recognize_saved_png(
    engine: &dyn OcrEngine,
    bytes: Vec<u8>,
    window: WindowIdentity,
    sequence: u64,
) -> Result<Vec<OcrItem>, VisionError> {
    let image = tokio::task::spawn_blocking(move || decode(bytes))
        .await
        .map_err(|_| VisionError::Protocol {
            message: "saved image decoder stopped".into(),
        })??;
    let roi = PhysicalRect {
        x: 0,
        y: 0,
        width: image.width,
        height: image.height,
    };
    let response = engine
        .recognize(OcrRequest {
            request_id: OcrRequestId::new(),
            window,
            frame_id: FrameId::new(sequence),
            topology_generation: TopologyGeneration::new(0),
            profile: OcrProfile::small(),
            roi,
            image,
            deadline: Duration::from_secs(30),
        })
        .await?;
    Ok(response.items)
}

fn decode(bytes: Vec<u8>) -> Result<PixelImage, VisionError> {
    let invalid = || VisionError::InvalidFrame {
        message: "saved screenshot is not an RGBA PNG".into(),
    };
    let mut reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|_| invalid())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or_else(invalid)?];
    let info = reader.next_frame(&mut pixels).map_err(|_| invalid())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(invalid());
    }
    pixels.truncate(info.buffer_size());
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(PixelImage::new(
        info.width,
        info.height,
        info.width as usize * 4,
        PixelFormat::Bgra8Unorm,
        pixels,
    )?)
}
