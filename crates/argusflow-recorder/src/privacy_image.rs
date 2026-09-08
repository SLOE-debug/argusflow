//! 截图隐私像素写入；遮盖图案与原像素无关，避免小字可被反推。
use crate::{PrivacyRect, RecorderError, ScreenshotKind};
use std::{io::Cursor, path::Path};

pub(crate) async fn mosaic(
    root: &Path,
    id: uuid::Uuid,
    sequence: u64,
    kind: ScreenshotKind,
    rect: PrivacyRect,
) -> Result<(), RecorderError> {
    let bytes = crate::evidence_reader::read(root, id, sequence, kind).await?;
    let recording = crate::history::load(root, id).await?;
    let evidence = recording
        .trace
        .timeline
        .events
        .iter()
        .find(|event| event.sequence == sequence)
        .and_then(|event| event.evidence.as_ref())
        .ok_or(RecorderError::InvalidRecording)?;
    let (shot, name) = match kind {
        ScreenshotKind::Window => (evidence.screenshot.as_ref(), sequence.to_string()),
        ScreenshotKind::Target => (evidence.click_target.as_ref(), format!("{sequence}-target")),
        _ => return Err(RecorderError::InvalidRecording),
    };
    let shot = shot.ok_or(RecorderError::InvalidRecording)?;
    let crop = shot.crop.as_ref().map(|crop| crop.bounds);
    let directory = tokio::fs::canonicalize(&recording.files.evidence_directory).await?;
    if !directory.starts_with(
        recording
            .files
            .timeline
            .parent()
            .ok_or(RecorderError::InvalidRecording)?,
    ) {
        return Err(RecorderError::InvalidRecording);
    }
    tokio::task::spawn_blocking(move || {
        let mut reader = png::Decoder::new(Cursor::new(bytes))
            .read_info()
            .map_err(|_| RecorderError::InvalidRecording)?;
        let mut pixels = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or(RecorderError::InvalidRecording)?
        ];
        let info = reader
            .next_frame(&mut pixels)
            .map_err(|_| RecorderError::InvalidRecording)?;
        if info.color_type != png::ColorType::Rgba
            || info.bit_depth != png::BitDepth::Eight
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|r| r > info.width)
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|b| b > info.height)
        {
            return Err(RecorderError::InvalidRecording);
        }
        for y in rect.y..rect.y + rect.height {
            for x in rect.x..rect.x + rect.width {
                let offset = ((y * info.width + x) * 4) as usize;
                let shade = if (x / 10 + y / 10) % 2 == 0 { 100 } else { 130 };
                pixels[offset..offset + 4].copy_from_slice(&[shade, shade, shade, 255]);
            }
        }
        write_png(
            &directory.join(format!("{name}.png")),
            info.width,
            info.height,
            &pixels,
        )?;
        if let Some(bounds) = crop {
            let (x, y, width, height) = (
                bounds.x as u32,
                bounds.y as u32,
                bounds.width as u32,
                bounds.height as u32,
            );
            if x.checked_add(width).is_none_or(|r| r > info.width)
                || y.checked_add(height).is_none_or(|b| b > info.height)
            {
                return Err(RecorderError::InvalidRecording);
            }
            let mut cropped = Vec::with_capacity((width * height * 4) as usize);
            for row in y..y + height {
                let start = ((row * info.width + x) * 4) as usize;
                cropped.extend_from_slice(&pixels[start..start + (width * 4) as usize]);
            }
            write_png(
                &directory.join(format!("{name}-crop.png")),
                width,
                height,
                &cropped,
            )?;
        }
        Ok(())
    })
    .await
    .map_err(|_| RecorderError::WorkerUnavailable)?
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), RecorderError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| RecorderError::InvalidRecording)?;
        writer
            .write_image_data(pixels)
            .map_err(|_| RecorderError::InvalidRecording)?;
    }
    let pending = path.with_extension("pending");
    std::fs::write(&pending, bytes)?;
    std::fs::rename(pending, path)?;
    Ok(())
}
