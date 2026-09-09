//! 演示包内 PNG 的受限读取；不接受前端或 JSON 中的任意文件路径。

use crate::{RecorderError, ScreenshotKind};
use std::path::Path;

pub(crate) async fn read(
    root: &Path,
    id: uuid::Uuid,
    sequence: u64,
    kind: ScreenshotKind,
) -> Result<Vec<u8>, RecorderError> {
    let recording = crate::history::load(root, id).await?;
    if matches!(kind, ScreenshotKind::Screen) {
        return tokio::task::spawn_blocking(move || {
            crate::screen_archive_reader::encode(&crate::screen_archive_reader::reconstruct(
                &recording.files.evidence_directory,
                &recording.trace.screen,
                crate::ScreenFrameId(sequence),
            )?)
        })
        .await
        .map_err(|_| RecorderError::WorkerUnavailable)?;
    }
    let screenshot = recording
        .trace
        .timeline
        .events
        .iter()
        .find(|event| event.sequence == sequence)
        .and_then(|event| event.evidence.as_ref())
        .and_then(|evidence| match kind {
            ScreenshotKind::Screen => None,
            ScreenshotKind::Target | ScreenshotKind::TargetCrop => evidence.click_target.as_ref(),
            ScreenshotKind::Window | ScreenshotKind::Crop => evidence.screenshot.as_ref(),
        })
        .ok_or(RecorderError::InvalidRecording)?;
    let name = match kind {
        ScreenshotKind::Screen => return Err(RecorderError::InvalidRecording),
        ScreenshotKind::Target => format!("{sequence}-target.png"),
        ScreenshotKind::TargetCrop if screenshot.crop.is_some() => {
            format!("{sequence}-target-crop.png")
        }
        ScreenshotKind::TargetCrop => return Err(RecorderError::InvalidRecording),
        ScreenshotKind::Window => format!("{sequence}.png"),
        ScreenshotKind::Crop if screenshot.crop.is_some() => format!("{sequence}-crop.png"),
        ScreenshotKind::Crop => return Err(RecorderError::InvalidRecording),
    };
    let root = tokio::fs::canonicalize(root).await?;
    let directory = tokio::fs::canonicalize(root.join(id.to_string()).join("evidence")).await?;
    let path = tokio::fs::canonicalize(directory.join(name)).await?;
    if !directory.starts_with(root.join(id.to_string())) || !path.starts_with(&directory) {
        return Err(RecorderError::InvalidRecording);
    }
    Ok(tokio::fs::read(path).await?)
}
