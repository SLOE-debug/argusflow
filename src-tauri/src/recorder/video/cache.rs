//! 小型磁盘映射把视频样本映射到内容寻址PNG；图片被预算淘汰后按需重建。
use super::index::Selected;
use argusflow_recorder::{Attachment, read_attachment};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
#[cfg(test)]
#[path = "../../../../tests/argusflow-desktop/unit/recorder/video_cache.rs"]
mod tests;

#[derive(Deserialize, Serialize)]
struct Cached {
    key: String,
    image: Attachment,
}
pub(super) struct Cache {
    entries: Vec<Cached>,
    key: String,
}
impl Cache {
    pub fn open(root: &Path, selected: &Selected) -> Result<Self, String> {
        let metadata = selected
            .directory
            .join("screen.mp4")
            .metadata()
            .map_err(|e| e.to_string())?;
        let key = format!(
            "{:?}:{}:{:?}:{}:{}x{}",
            selected.directory.file_name(),
            metadata.len(),
            metadata.modified().map_err(|e| e.to_string())?,
            selected.entry.frame.pts_100ns,
            selected.header.width,
            selected.header.height
        );
        let path = root.join("review-frames.json");
        let entries = if path.exists() {
            if path.metadata().map_err(|e| e.to_string())?.len() > 65536 {
                return Err("回看映射超限".into());
            }
            let entries: Vec<Cached> =
                serde_json::from_reader(std::fs::File::open(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if entries.len() > 32 {
                return Err("回看映射数量超限".into());
            }
            entries
        } else {
            vec![]
        };
        Ok(Self { entries, key })
    }
    pub fn get(&self, root: &Path) -> Result<Option<(Attachment, Vec<u8>)>, String> {
        let Some(entry) = self.entries.iter().find(|e| e.key == self.key) else {
            return Ok(None);
        };
        match read_attachment(root, &entry.image) {
            Ok(bytes) => Ok(Some((entry.image.clone(), bytes))),
            Err(argusflow_recorder::StorageError::Io(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn put(mut self, root: &Path, image: &Attachment) -> Result<(), String> {
        self.entries
            .retain(|e| e.key != self.key && root.join(&e.image.path).is_file());
        while self.entries.len() >= 32 {
            self.entries.remove(0);
        }
        self.entries.push(Cached {
            key: self.key,
            image: image.clone(),
        });
        let mut temporary = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
        temporary
            .write_all(&serde_json::to_vec(&self.entries).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        temporary
            .persist(root.join("review-frames.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
