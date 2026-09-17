//! 数据包发现、只读分页与非活动恢复。
use argusflow_recorder::*;
use serde::Serialize;
use std::path::Path;
#[derive(Serialize)]
pub(crate) struct RecordingEntry {
    pub session: Session,
    pub directory: String,
}
pub(super) fn list(root: &Path) -> Result<Vec<RecordingEntry>, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut entries = vec![];
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir()
            && let Ok(session) = load_session(&entry.path())
        {
            entries.push(RecordingEntry {
                session,
                directory: entry.path().to_string_lossy().into_owned(),
            });
        }
        if entries.len() >= 1000 {
            break;
        }
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.session.created_ms));
    Ok(entries)
}
