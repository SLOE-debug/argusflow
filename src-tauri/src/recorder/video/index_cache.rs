//! 仅保留一个片段的紧凑索引；JSONL仍是磁盘事实源，长度/修改时间变化即重建。
use super::index::{Entry, Header};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

pub(super) struct TimedEntry {
    pub entry: Entry,
    pub at: i64,
    pub end: i64,
}
#[derive(Default)]
pub(super) struct IndexCache {
    identity: Option<(PathBuf, u64, SystemTime, i64, u64)>,
    entries: Vec<TimedEntry>,
}
impl IndexCache {
    pub fn entries(&mut self, directory: &Path, header: &Header) -> Result<&[TimedEntry], String> {
        let path = directory.join("frames.jsonl");
        let metadata = path.metadata().map_err(|_| "视频帧正在写入，请稍后重试")?;
        let identity = (
            path.clone(),
            metadata.len(),
            metadata.modified().map_err(|e| e.to_string())?,
            header.qpc_origin,
            header.qpc_frequency,
        );
        if self.identity.as_ref() == Some(&identity) {
            return Ok(&self.entries);
        }
        self.identity = None;
        self.entries.clear();
        let mut reader = BufReader::new(std::fs::File::open(path).map_err(|e| e.to_string())?);
        let mut line = String::new();
        loop {
            line.clear();
            if (&mut reader)
                .take(4097)
                .read_line(&mut line)
                .map_err(|e| e.to_string())?
                == 0
            {
                break;
            }
            if line.len() > 4096 {
                return Err("视频索引行超限".into());
            }
            if !line.ends_with('\n') {
                break;
            }
            if self.entries.len() >= 216_000 {
                return Err("视频索引帧数超限".into());
            }
            let entry: Entry = serde_json::from_str(&line).map_err(|e| e.to_string())?;
            if entry.duration_100ns <= 0
                || entry.duration_100ns > 36_000_010_000
                || !(0..=36_000_000_000).contains(&entry.frame.pts_100ns)
            {
                return Err("视频帧时长无效".into());
            }
            let time = |pts| {
                i64::try_from(
                    i128::from(header.qpc_origin)
                        + i128::from(pts) * i128::from(header.qpc_frequency) / 10_000_000,
                )
                .map_err(|_| "视频时钟溢出".to_string())
            };
            let at = time(entry.frame.pts_100ns)?;
            let end = time(entry.frame.pts_100ns + entry.duration_100ns)?;
            if self
                .entries
                .last()
                .is_some_and(|previous| previous.at >= at)
            {
                return Err("视频索引时间没有递增".into());
            }
            self.entries.push(TimedEntry { entry, at, end });
        }
        self.identity = Some(identity);
        Ok(&self.entries)
    }
}
