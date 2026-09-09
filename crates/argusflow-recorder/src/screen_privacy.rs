//! 屏幕隐私编辑先重建依赖，再发布新检查点，最后删除被替代像素。

use crate::{
    PrivacyEdit, PrivacyRect, RecorderError, ScreenFrameId, ScreenPatch, ScreenTimeline,
    ScreenshotKind,
};
use std::{collections::BTreeMap, path::Path};

/// 校验帧身份和局部范围，同时将事件抹除展开到其全部视觉引用。
pub(crate) fn masks(
    trace: &crate::RecordingTrace,
    edits: &[PrivacyEdit],
) -> Result<BTreeMap<ScreenFrameId, Vec<PrivacyRect>>, RecorderError> {
    let mut masks: BTreeMap<ScreenFrameId, Vec<PrivacyRect>> = BTreeMap::new();
    for edit in edits {
        match edit {
            PrivacyEdit::Mosaic {
                sequence,
                kind: ScreenshotKind::Screen,
                rect,
            } => {
                let id = ScreenFrameId(*sequence);
                let frame = trace
                    .screen
                    .frames
                    .iter()
                    .find(|frame| frame.id == id)
                    .ok_or(RecorderError::InvalidRecording)?;
                if rect.width == 0
                    || rect.height == 0
                    || rect
                        .x
                        .checked_add(rect.width)
                        .is_none_or(|right| f64::from(right) > frame.bounds.width)
                    || rect
                        .y
                        .checked_add(rect.height)
                        .is_none_or(|bottom| f64::from(bottom) > frame.bounds.height)
                {
                    return Err(RecorderError::InvalidRecording);
                }
                masks.entry(id).or_default().push(*rect);
            }
            PrivacyEdit::EraseEvent { sequence } => {
                let event = trace
                    .timeline
                    .events
                    .iter()
                    .find(|event| event.sequence == *sequence)
                    .ok_or(RecorderError::InvalidRecording)?;
                if let Some(evidence) = &event.evidence {
                    for id in evidence.screen.before.iter().chain(&evidence.screen.after) {
                        let frame = trace
                            .screen
                            .frames
                            .iter()
                            .find(|frame| frame.id == *id)
                            .ok_or(RecorderError::InvalidRecording)?;
                        masks.entry(*id).or_default().push(PrivacyRect {
                            x: 0,
                            y: 0,
                            width: frame.bounds.width as u32,
                            height: frame.bounds.height as u32,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(masks)
}

/// 受影响帧及其增量后继成为独立检查点，避免覆盖共享前驱破坏后继像素。
pub(crate) fn stage(
    directory: &Path,
    timeline: &mut ScreenTimeline,
    masks: &BTreeMap<ScreenFrameId, Vec<PrivacyRect>>,
) -> Result<Vec<std::path::PathBuf>, RecorderError> {
    let original = timeline.clone();
    let mut replaced = std::collections::BTreeSet::new();
    let mut obsolete = Vec::new();
    for record in &mut timeline.frames {
        if !masks.contains_key(&record.id)
            && !record.previous.is_some_and(|id| replaced.contains(&id))
        {
            continue;
        }
        let frame = crate::screen_archive_reader::reconstruct(directory, &original, record.id)?
            .into_rgba8();
        let mut pixels = frame.pixels().to_vec();
        if let Some(regions) = masks.get(&record.id) {
            for rect in regions {
                for y in rect.y..rect.y + rect.height {
                    for x in rect.x..rect.x + rect.width {
                        let offset = ((y * frame.width() + x) * 4) as usize;
                        let shade = if (x / 10 + y / 10) % 2 == 0 { 100 } else { 130 };
                        pixels[offset..offset + 4].copy_from_slice(&[shade, shade, shade, 255]);
                    }
                }
            }
        }
        let edited = argusflow_core::EvidenceFrame::new(
            frame.bounds(),
            frame.width(),
            frame.height(),
            argusflow_core::EvidencePixelFormat::Rgba8,
            pixels,
        )
        .map_err(|_| RecorderError::InvalidRecording)?;
        // 每次编辑使用未引用的新区域身份；失败时原清单仍能完整读取。
        let index = record
            .patches
            .iter()
            .map(|patch| patch.index)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RecorderError::InvalidRecording)?;
        let target = directory.join(crate::screen_archive::patch_name(record.id, index));
        std::fs::write(&target, crate::screen_archive_reader::encode(&edited)?)?;
        for patch in &record.patches {
            obsolete
                .push(directory.join(crate::screen_archive::patch_name(record.id, patch.index)));
        }
        record.previous = None;
        record.patches = vec![ScreenPatch {
            index,
            bounds: record.bounds,
        }];
        replaced.insert(record.id);
    }
    Ok(obsolete)
}
