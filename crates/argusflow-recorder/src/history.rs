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
    /// 原始事件数。
    pub raw_event_count: usize,
    /// 规范化步骤数。
    pub semantic_record_count: usize,
    /// 捕获或处理阶段丢弃数。
    pub dropped_events: u64,
}

impl RecordingSummary {
    pub(crate) fn from_trace(trace: &RecordingTrace) -> Self {
        Self {
            schema_version: trace.schema_version,
            recording_id: trace.recording_id,
            started_at_unix_ms: trace.started_at_unix_ms,
            duration_ms: trace.raw.events.last().map_or(0, |event| event.elapsed_ms),
            raw_event_count: trace.raw.events.len(),
            semantic_record_count: trace.normalized.records.len(),
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
        let summary: RecordingSummary = serde_json::from_slice(&tokio::fs::read(manifest).await?)?;
        validate_identity(&summary, id)?;
        summaries.push(summary);
    }
    summaries.sort_by_key(|summary| std::cmp::Reverse(summary.started_at_unix_ms));
    summaries.truncate(100);
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
        raw: directory.join("raw.json"),
        normalized: directory.join("semantic.json"),
        manifest: directory.join("manifest.json"),
    };
    let summary: RecordingSummary =
        serde_json::from_slice(&tokio::fs::read(&files.manifest).await?)?;
    validate_identity(&summary, id)?;
    let trace = RecordingTrace {
        schema_version: summary.schema_version,
        recording_id: id,
        started_at_unix_ms: summary.started_at_unix_ms,
        raw: serde_json::from_slice(&tokio::fs::read(&files.raw).await?)?,
        normalized: serde_json::from_slice(&tokio::fs::read(&files.normalized).await?)?,
        dropped_events: summary.dropped_events,
    };
    if trace.raw.events.len() != summary.raw_event_count
        || trace.normalized.records.len() != summary.semantic_record_count
    {
        return Err(RecorderError::InvalidRecording);
    }
    Ok(CompletedRecording { files, trace })
}

fn validate_identity(summary: &RecordingSummary, id: uuid::Uuid) -> Result<(), RecorderError> {
    if summary.recording_id != id || summary.schema_version != 1 {
        return Err(RecorderError::InvalidRecording);
    }
    Ok(())
}
