//! 停止后顺序读取候选归档，以 GPU 比较真实差分并暂存新区域。
use crate::{
    RecorderError, ScreenFrame, ScreenFrameId, ScreenPatch, ScreenTimeline,
    screen_archive::{ScreenRefinement, patch_name},
};
use argusflow_capture::{FrameSnapshot, PixelRect};
use argusflow_core::{CaptureGeneration, InspectionRect, capture::refinement::PixelDiffer};
use std::path::{Path, PathBuf};

/// 新索引发布成功后才允许删除旧像素；失败时只回收本次创建的新文件。
pub(crate) struct RefinedArchive {
    pub timeline: ScreenTimeline,
    pub obsolete: Vec<PathBuf>,
    created: Vec<PathBuf>,
}
impl RefinedArchive {
    pub fn commit(mut self) -> (ScreenTimeline, Vec<PathBuf>) {
        self.created.clear();
        (self.timeline.clone(), std::mem::take(&mut self.obsolete))
    }
}
impl Drop for RefinedArchive {
    fn drop(&mut self) {
        for path in &self.created {
            let _ = std::fs::remove_file(path);
        }
    }
}

struct SourceState {
    snapshot: FrameSnapshot,
    generation: CaptureGeneration,
    last: ScreenFrameId,
    checkpoint_us: u64,
    changes: u32,
}

/// 仅保留一个来源的两份快照；帧 ID 和原始呈现时间保持不变。
pub(crate) fn refine(
    directory: &Path,
    raw: &ScreenTimeline,
    differ: &mut dyn PixelDiffer,
) -> Result<RefinedArchive, RecorderError> {
    let mut staged = RefinedArchive {
        timeline: ScreenTimeline {
            frames: Vec::new(),
            ..raw.clone()
        },
        obsolete: Vec::new(),
        created: Vec::new(),
    };
    let mut ordered: Vec<_> = raw.frames.iter().collect();
    ordered.sort_by_key(|frame| (frame.source.0, frame.id));
    let mut source = None;
    let mut state: Option<SourceState> = None;
    for record in ordered {
        if source != Some(record.source) {
            source = Some(record.source);
            state = None;
        }
        let patches = crate::screen_archive_reader::read_patches(directory, record)?;
        let reset = state.as_ref().is_none_or(|old| {
            old.generation != record.generation || old.snapshot.bounds() != record.bounds
        });
        let snapshot = if record.previous.is_none() {
            if patches.len() != 1 || patches[0].bounds() != record.bounds {
                return Err(RecorderError::InvalidRecording);
            }
            FrameSnapshot::from_frame(&patches[0])
        } else {
            if reset {
                return Err(RecorderError::InvalidRecording);
            }
            state
                .as_ref()
                .ok_or(RecorderError::InvalidRecording)?
                .snapshot
                .apply_candidates(&patches)
                .map_err(|_| RecorderError::InvalidRecording)?
        };
        let mut changes = Vec::new();
        if reset {
            changes.push(record.bounds);
        } else {
            let old = &state
                .as_ref()
                .ok_or(RecorderError::InvalidRecording)?
                .snapshot;
            // 检查点可能复制全屏，但仍只需验证这次原生更新报告的候选区。
            for candidate in &record.changes {
                let region = local(record.bounds, *candidate)?;
                let before = old
                    .crop(region)
                    .map_err(|_| RecorderError::InvalidRecording)?;
                let after = snapshot
                    .crop(region)
                    .map_err(|_| RecorderError::InvalidRecording)?;
                changes.extend(
                    differ
                        .compare(&before, &after)
                        .map_err(|error| RecorderError::Refinement(error.to_string()))?,
                );
            }
        }
        staged.obsolete.extend(
            record
                .patches
                .iter()
                .map(|patch| directory.join(patch_name(record.id, patch.index))),
        );
        if changes.is_empty() {
            if let Some(old) = &mut state {
                old.snapshot = snapshot;
            }
            continue;
        }
        let checkpoint = reset
            || state.as_ref().is_none_or(|old| {
                old.changes >= 120
                    || record.presented_us.saturating_sub(old.checkpoint_us) >= 2_000_000
            });
        let mut precise = ScreenFrame {
            previous: if checkpoint {
                None
            } else {
                state.as_ref().map(|old| old.last)
            },
            changes,
            patches: Vec::new(),
            ..record.clone()
        };
        let regions = if checkpoint {
            vec![record.bounds]
        } else {
            precise.changes.clone()
        };
        let first = record
            .patches
            .iter()
            .map(|patch| patch.index)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(RecorderError::InvalidRecording)?;
        for (offset, bounds) in regions.into_iter().enumerate() {
            let index = first
                .checked_add(u32::try_from(offset).map_err(|_| RecorderError::InvalidRecording)?)
                .ok_or(RecorderError::InvalidRecording)?;
            let target = directory.join(patch_name(record.id, index));
            // create_new 拒绝覆盖任何旧像素或之前未提交的后处理结果。
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            staged.created.push(target);
            let pixels = snapshot
                .crop(local(record.bounds, bounds)?)
                .map_err(|_| RecorderError::InvalidRecording)?;
            use std::io::Write;
            file.write_all(&crate::screen_archive_reader::encode(&pixels)?)?;
            precise.patches.push(ScreenPatch { index, bounds });
        }
        state = Some(SourceState {
            snapshot,
            generation: record.generation,
            last: record.id,
            checkpoint_us: if checkpoint {
                record.presented_us
            } else {
                state
                    .as_ref()
                    .map_or(record.presented_us, |old| old.checkpoint_us)
            },
            changes: if checkpoint {
                0
            } else {
                state.as_ref().map_or(0, |old| old.changes + 1)
            },
        });
        staged.timeline.frames.push(precise);
    }
    staged.timeline.frames.sort_by_key(|frame| frame.id);
    staged.timeline.refinement = ScreenRefinement::Complete;
    Ok(staged)
}

fn local(frame: InspectionRect, region: InspectionRect) -> Result<PixelRect, RecorderError> {
    let x = region.x - frame.x;
    let y = region.y - frame.y;
    if !region.is_valid()
        || x < 0.0
        || y < 0.0
        || x.fract() != 0.0
        || y.fract() != 0.0
        || region.width.fract() != 0.0
        || region.height.fract() != 0.0
        || x + region.width > frame.width
        || y + region.height > frame.height
    {
        return Err(RecorderError::InvalidRecording);
    }
    Ok(PixelRect {
        x: x as u32,
        y: y as u32,
        width: region.width as u32,
        height: region.height as u32,
    })
}

#[cfg(test)]
#[path = "screen_refinement_tests.rs"]
mod tests;
