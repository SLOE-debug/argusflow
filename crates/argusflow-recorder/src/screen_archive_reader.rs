//! 增量链的受限读取和像素重建；不接受调用方文件路径。

use crate::{RecorderError, ScreenFrameId, ScreenTimeline, screen_archive::patch_name};
use argusflow_capture::FrameSnapshot;
use argusflow_core::{EvidenceFrame, EvidencePixelFormat};
use std::path::Path;

/// 校验来源与依赖后重建完整帧。
pub(crate) fn reconstruct(
    directory: &Path,
    timeline: &ScreenTimeline,
    id: ScreenFrameId,
) -> Result<EvidenceFrame, RecorderError> {
    let mut chain = Vec::new();
    let mut cursor = id;
    loop {
        let frame = timeline
            .frames
            .iter()
            .find(|frame| frame.id == cursor)
            .ok_or(RecorderError::InvalidRecording)?;
        if chain.len() > 120 {
            return Err(RecorderError::InvalidRecording);
        }
        chain.push(frame);
        match frame.previous {
            Some(previous) if previous < frame.id => cursor = previous,
            Some(_) => return Err(RecorderError::InvalidRecording),
            None => break,
        }
    }
    chain.reverse();
    let base = *chain.first().ok_or(RecorderError::InvalidRecording)?;
    let mut snapshot: Option<FrameSnapshot> = None;
    for frame in chain {
        if frame.source != base.source
            || frame.generation != base.generation
            || frame.bounds != base.bounds
        {
            return Err(RecorderError::InvalidRecording);
        }
        let patches = read_patches(directory, frame)?;
        snapshot = Some(match snapshot {
            None => {
                if patches.len() != 1 || patches[0].bounds() != frame.bounds {
                    return Err(RecorderError::InvalidRecording);
                }
                FrameSnapshot::from_frame(&patches[0])
            }
            Some(previous) => previous
                .apply_candidates(&patches)
                .map_err(|_| RecorderError::InvalidRecording)?,
        });
    }
    snapshot
        .ok_or(RecorderError::InvalidRecording)?
        .materialize()
        .map_err(|_| RecorderError::InvalidRecording)
}

/// 一次解码当前帧区域，顺序后处理无需反复回溯检查点。
pub(crate) fn read_patches(
    directory: &Path,
    frame: &crate::ScreenFrame,
) -> Result<Vec<EvidenceFrame>, RecorderError> {
    frame
        .patches
        .iter()
        .map(|patch| {
            let path = std::fs::canonicalize(directory.join(patch_name(frame.id, patch.index)))?;
            let root = std::fs::canonicalize(directory)?;
            if !path.starts_with(root) {
                return Err(RecorderError::InvalidRecording);
            }
            let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?))
                .read_info()
                .map_err(|_| RecorderError::InvalidRecording)?;
            let size = reader
                .output_buffer_size()
                .filter(|size| *size <= 128 * 1024 * 1024)
                .ok_or(RecorderError::InvalidRecording)?;
            let mut pixels = vec![0; size];
            let info = reader
                .next_frame(&mut pixels)
                .map_err(|_| RecorderError::InvalidRecording)?;
            if info.color_type != png::ColorType::Rgba
                || info.bit_depth != png::BitDepth::Eight
                || patch.bounds.width != f64::from(info.width)
                || patch.bounds.height != f64::from(info.height)
            {
                return Err(RecorderError::InvalidRecording);
            }
            pixels.truncate(info.buffer_size());
            EvidenceFrame::new(
                patch.bounds,
                info.width,
                info.height,
                EvidencePixelFormat::Rgba8,
                pixels,
            )
            .map_err(|_| RecorderError::InvalidRecording)
        })
        .collect()
}

/// 输出供现有图像预览使用的无损完整 PNG。
pub(crate) fn encode(frame: &EvidenceFrame) -> Result<Vec<u8>, RecorderError> {
    let frame = frame.clone().into_rgba8();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, frame.width(), frame.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| RecorderError::InvalidRecording)?;
        writer
            .write_image_data(frame.pixels())
            .map_err(|_| RecorderError::InvalidRecording)?;
        writer
            .finish()
            .map_err(|_| RecorderError::InvalidRecording)?;
    }
    Ok(bytes)
}

impl crate::RecorderService {
    /// 区域只允许使用已发布索引中的标识，不能指定文件路径。
    pub async fn screen_region(
        &self,
        id: uuid::Uuid,
        frame: ScreenFrameId,
        region: Option<u32>,
    ) -> Result<Vec<u8>, RecorderError> {
        let _session = self.session.lock().await;
        let recording = crate::history::load(&self.root, id).await?;
        tokio::task::spawn_blocking(move || {
            let pixels = reconstruct(
                &recording.files.evidence_directory,
                &recording.trace.screen,
                frame,
            )?;
            let pixels = if let Some(index) = region {
                let record = recording
                    .trace
                    .screen
                    .frames
                    .iter()
                    .find(|candidate| candidate.id == frame)
                    .ok_or(RecorderError::InvalidRecording)?;
                let patch = record
                    .patches
                    .iter()
                    .find(|patch| patch.index == index)
                    .ok_or(RecorderError::InvalidRecording)?;
                FrameSnapshot::from_frame(&pixels)
                    .crop(argusflow_capture::PixelRect {
                        x: (patch.bounds.x - record.bounds.x) as u32,
                        y: (patch.bounds.y - record.bounds.y) as u32,
                        width: patch.bounds.width as u32,
                        height: patch.bounds.height as u32,
                    })
                    .map_err(|_| RecorderError::InvalidRecording)?
            } else {
                pixels
            };
            encode(&pixels)
        })
        .await
        .map_err(|_| RecorderError::WorkerUnavailable)?
    }
    /// 只接受录制身份和已归档帧身份。
    pub async fn screen_frame(
        &self,
        id: uuid::Uuid,
        frame: ScreenFrameId,
    ) -> Result<Vec<u8>, RecorderError> {
        let recording = crate::history::load(&self.root, id).await?;
        tokio::task::spawn_blocking(move || {
            encode(&reconstruct(
                &recording.files.evidence_directory,
                &recording.trace.screen,
                frame,
            )?)
        })
        .await
        .map_err(|_| RecorderError::WorkerUnavailable)?
    }
}
