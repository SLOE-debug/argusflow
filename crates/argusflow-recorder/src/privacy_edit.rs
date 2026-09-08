//! 已保存事件的隐私编辑；仅接受事件身份和帧本地矩形。

use crate::{CompletedRecording, RecorderError, RecorderService, ScreenshotKind};
use serde::{Deserialize, Serialize};

/// 截图原始像素坐标中的非空矩形，写入前校验范围。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct PrivacyRect {
    /// 左边界，单位像素。
    pub x: u32,
    /// 上边界，单位像素。
    pub y: u32,
    /// 宽度，单位像素。
    pub width: u32,
    /// 高度，单位像素。
    pub height: u32,
}

/// 事件抹除与截图遮盖的封闭操作契约。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrivacyEdit {
    /// 移除整个事件及其所有图像文件。
    EraseEvent {
        /// 时间线事件序号。
        sequence: u64,
    },
    /// 将区域替换为不含原像素的马赛克，并同步更新局部图。
    Mosaic {
        /// 时间线事件序号。
        sequence: u64,
        /// 只允许完整窗口或点击目标帧。
        kind: ScreenshotKind,
        /// 原图坐标矩形。
        rect: PrivacyRect,
    },
}

impl RecorderService {
    /// 串行编辑已保存录制；预先校验整批事件与图像，拒绝修改当前录制。
    pub async fn edit_privacy(
        &self,
        id: uuid::Uuid,
        edits: Vec<PrivacyEdit>,
    ) -> Result<CompletedRecording, RecorderError> {
        let _session = self.session.lock().await;
        if _session.as_ref().is_some_and(|active| active.id == id)
            || edits.is_empty()
            || edits.len() > 10_000
        {
            return Err(RecorderError::InvalidRecording);
        }
        apply(&self.root, id, edits).await
    }
}

pub(crate) async fn apply(
    root: &std::path::Path,
    id: uuid::Uuid,
    edits: Vec<PrivacyEdit>,
) -> Result<CompletedRecording, RecorderError> {
    let mut recording = crate::history::load(root, id).await?;
    // 完整验证后才写盘，防止批量中后面的非法序号导致前面的内容被修改。
    for edit in &edits {
        let sequence = match edit {
            PrivacyEdit::EraseEvent { sequence } | PrivacyEdit::Mosaic { sequence, .. } => sequence,
        };
        let event = recording
            .trace
            .timeline
            .events
            .iter()
            .find(|event| event.sequence == *sequence)
            .ok_or(RecorderError::InvalidRecording)?;
        if let PrivacyEdit::Mosaic { kind, rect, .. } = edit {
            let evidence = event
                .evidence
                .as_ref()
                .ok_or(RecorderError::InvalidRecording)?;
            let shot = match kind {
                ScreenshotKind::Window => evidence.screenshot.as_ref(),
                ScreenshotKind::Target => evidence.click_target.as_ref(),
                _ => return Err(RecorderError::InvalidRecording),
            }
            .ok_or(RecorderError::InvalidRecording)?;
            if rect.width == 0
                || rect.height == 0
                || rect
                    .x
                    .checked_add(rect.width)
                    .is_none_or(|right| right > shot.width)
                || rect
                    .y
                    .checked_add(rect.height)
                    .is_none_or(|bottom| bottom > shot.height)
            {
                return Err(RecorderError::InvalidRecording);
            }
        }
    }
    for edit in &edits {
        if let PrivacyEdit::Mosaic {
            sequence,
            kind,
            rect,
        } = edit
        {
            crate::privacy_image::mosaic(root, id, *sequence, *kind, *rect).await?;
        }
    }
    // 固定文件名覆盖所有截图变体，不能只移除 JSON 引用而遗留原图。
    for edit in &edits {
        if let PrivacyEdit::EraseEvent { sequence } = edit {
            for suffix in ["", "-crop", "-target", "-target-crop"] {
                let path = recording
                    .files
                    .evidence_directory
                    .join(format!("{sequence}{suffix}.png"));
                if tokio::fs::try_exists(&path).await? {
                    let resolved = tokio::fs::canonicalize(&path).await?;
                    if !resolved.starts_with(&recording.files.evidence_directory) {
                        return Err(RecorderError::InvalidRecording);
                    }
                    tokio::fs::remove_file(path).await?;
                }
            }
        }
    }
    recording.trace.timeline.events.retain(|event| {
        !edits.iter().any(|edit|
            matches!(edit, PrivacyEdit::EraseEvent { sequence } if *sequence == event.sequence))
    });
    recording.files = crate::storage::save(root, &recording.trace).await?;
    Ok(recording)
}
