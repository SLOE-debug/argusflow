//! 本地 Trace 存储；raw/normalized 分文件，完成标记最后写入。

use crate::{RecorderError, RecordingTrace};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 已持久化文件的位置，供 Tauri/CLI 读取或导出。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingFiles {
    /// 已脱敏 Raw Trace JSON。
    pub raw: PathBuf,
    /// 可直接发送给 AI 的语义 JSON。
    pub normalized: PathBuf,
    /// 最后写入的版本/录制 ID 元数据。
    pub manifest: PathBuf,
}

/// Store 的根目录由宿主配置；文件名仅由 UUID 产生，不接受任意输入路径。
pub(crate) async fn save(
    root: &Path,
    trace: &RecordingTrace,
) -> Result<RecordingFiles, RecorderError> {
    let directory = root.join(trace.recording_id.to_string());
    tokio::fs::create_dir_all(&directory).await?;
    let directory = tokio::fs::canonicalize(directory).await?;
    let files = RecordingFiles {
        raw: directory.join("raw.json"),
        normalized: directory.join("semantic.json"),
        manifest: directory.join("manifest.json"),
    };
    write_json(&files.raw, &trace.raw).await?;
    write_json(&files.normalized, &trace.normalized).await?;
    let metadata = crate::history::RecordingSummary::from_trace(trace);
    write_json(&files.manifest, &metadata).await?;
    Ok(files)
}

/// 完整 JSON 先写入 .pending，再以单文件 rename 发布，避免读到半截输入。
async fn write_json(path: &Path, value: &impl Serialize) -> Result<(), RecorderError> {
    let pending = path.with_extension("pending");
    let bytes = serde_json::to_vec_pretty(value)?;
    tokio::fs::write(&pending, bytes).await?;
    tokio::fs::rename(&pending, path).await?;
    Ok(())
}
