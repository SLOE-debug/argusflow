//! 已发布录制的索引和按 ID 读取；不接受任意文件路径。

use crate::{CompletedRecording, RecorderError, RecordingFiles, RecordingTrace};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Manifest 也是历史列表摘要；不包含输入内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSummary {
    /// Trace 协议版本。
    pub schema_version: u16,
    /// 用于目录定位的录制身份。
    pub recording_id: uuid::Uuid,
    /// 开始的 Unix 毫秒时间。
    pub started_at_unix_ms: u64,
    /// 最后一个事件相对于开始的毫秒数。
    pub duration_ms: u64,
    /// 精简后的时间线条目数，移动段计为一条。
    pub event_count: usize,
    /// 保存了完整窗口图像证据的事件数量。
    pub screenshot_count: usize,
    /// 捕获或处理阶段丢弃数。
    pub dropped_events: u64,
}

impl RecordingSummary {
    pub(crate) fn from_trace(trace: &RecordingTrace) -> Self {
        Self {
            schema_version: trace.schema_version,
            recording_id: trace.recording_id,
            started_at_unix_ms: trace.started_at_unix_ms,
            duration_ms: trace
                .timeline
                .events
                .iter()
                .map(|event| event.ended_ms())
                .max()
                .unwrap_or(0),
            event_count: trace.timeline.events.len(),
            screenshot_count: trace
                .timeline
                .events
                .iter()
                .filter(|event| {
                    event
                        .evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.screenshot.is_some())
                })
                .count(),
            dropped_events: trace.dropped_events,
        }
    }
}

pub(crate) async fn list(root: &Path) -> Result<Vec<RecordingSummary>, RecorderError> {
    if !tokio::fs::try_exists(root).await? {
        return Ok(Vec::new());
    }
    let mut entries = tokio::fs::read_dir(root).await?;
    let mut summaries = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let Ok(id) = uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let manifest = entry.path().join("manifest.json");
        if !tokio::fs::try_exists(&manifest).await? {
            continue;
        }
        // 历史列表只列当前协议，不为旧 Semantic Trace 补转换或双写。
        let Ok(summary) =
            serde_json::from_slice::<RecordingSummary>(&tokio::fs::read(manifest).await?)
        else {
            continue;
        };
        if summary.schema_version != 2 {
            continue;
        }
        validate_identity(&summary, id)?;
        summaries.push(summary);
    }
    summaries.sort_by_key(|summary| std::cmp::Reverse(summary.started_at_unix_ms));
    summaries.truncate(100);
    // 历史与新录制使用同一轨迹规则；不重写用户已保存的原始文件。
    for summary in &mut summaries {
        let recording = load(root, summary.recording_id).await?;
        *summary = RecordingSummary::from_trace(&recording.trace);
    }
    Ok(summaries)
}

pub(crate) async fn load(root: &Path, id: uuid::Uuid) -> Result<CompletedRecording, RecorderError> {
    let root = tokio::fs::canonicalize(root).await?;
    let directory = tokio::fs::canonicalize(root.join(id.to_string())).await?;
    // 拒绝目录 junction/symlink 将 UUID 读取越出录制根目录。
    if !directory.starts_with(&root) {
        return Err(RecorderError::InvalidRecording);
    }
    let files = RecordingFiles {
        timeline: directory.join("timeline.json"),
        evidence_directory: directory.join("evidence"),
        manifest: directory.join("manifest.json"),
    };
    let summary: RecordingSummary =
        serde_json::from_slice(&tokio::fs::read(&files.manifest).await?)?;
    validate_identity(&summary, id)?;
    let mut trace = RecordingTrace {
        schema_version: summary.schema_version,
        recording_id: id,
        started_at_unix_ms: summary.started_at_unix_ms,
        timeline: serde_json::from_slice(&tokio::fs::read(&files.timeline).await?)?,
        dropped_events: summary.dropped_events,
    };
    if trace.timeline.events.len() != summary.event_count
        || RecordingSummary::from_trace(&trace).screenshot_count != summary.screenshot_count
    {
        return Err(RecorderError::InvalidRecording);
    }
    // 先校验磁盘上的完整记录，再派生精简时间线，已有截图引用和原文件不变。
    trace.timeline.compact_pointer_motion();
    Ok(CompletedRecording { files, trace })
}

fn validate_identity(summary: &RecordingSummary, id: uuid::Uuid) -> Result<(), RecorderError> {
    if summary.recording_id != id || summary.schema_version != 2 {
        return Err(RecorderError::InvalidRecording);
    }
    Ok(())
}
