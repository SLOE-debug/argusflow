//! 有界JSON派生结果；不缓存视频像素，重启后可按源文件身份复用。
use super::model::Decision;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
#[cfg(test)]
#[path = "../../../../../tests/argusflow-desktop/unit/recorder/video_analysis_cache.rs"]
mod tests;
#[derive(Deserialize, Serialize)]
struct Entry {
    key: String,
    decision: Decision,
}
pub(super) struct Cache {
    key: String,
    entries: Vec<Entry>,
}
impl Cache {
    pub fn open(
        root: &Path,
        video: &Path,
        anchor: i64,
        target: i64,
        earliest: i64,
    ) -> Result<Self, String> {
        let metadata = video.metadata().map_err(|e| e.to_string())?;
        let index = video
            .with_file_name("frames.jsonl")
            .metadata()
            .map_err(|e| e.to_string())?;
        let header = video
            .with_file_name("session.json")
            .metadata()
            .map_err(|e| e.to_string())?;
        let key = format!(
            "pixels-v2:{:?}:{}:{:?}:{}:{:?}:{}:{:?}:{anchor}:{target}:{earliest}",
            video,
            metadata.len(),
            metadata.modified().map_err(|e| e.to_string())?,
            index.len(),
            index.modified().map_err(|e| e.to_string())?,
            header.len(),
            header.modified().map_err(|e| e.to_string())?
        );
        let path = root.join("review-analysis.json");
        let entries = if path.exists() {
            if path.metadata().map_err(|e| e.to_string())?.len() > 512 * 1024 {
                return Err("分析缓存超限".into());
            }
            let entries: Vec<Entry> =
                serde_json::from_reader(std::fs::File::open(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if entries.len() > 128 {
                return Err("分析缓存数量超限".into());
            }
            entries
        } else {
            vec![]
        };
        Ok(Self { key, entries })
    }
    pub fn get(&self) -> Option<Decision> {
        self.entries
            .iter()
            .find(|entry| entry.key == self.key)
            .map(|entry| entry.decision.clone())
    }
    pub fn put(mut self, root: &Path, decision: Decision) -> Result<(), String> {
        self.entries.retain(|entry| entry.key != self.key);
        while self.entries.len() >= 128 {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            key: self.key,
            decision,
        });
        let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(&self.entries).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.persist(root.join("review-analysis.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
