//! 在有界元数据索引上二分查找，不缓存整段录像或像素。
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[cfg(test)]
#[path = "../../../../tests/argusflow-desktop/unit/recorder/video_index.rs"]
mod tests;
#[derive(Deserialize)]
pub(super) struct Header {
    pub qpc_origin: i64,
    pub qpc_frequency: u64,
    pub width: u32,
    pub height: u32,
    pub source: Source,
}
#[derive(Deserialize)]
pub(super) struct Source {
    pub origin: [i32; 2],
    pub dpi: [u32; 2],
}
#[derive(Clone, Deserialize)]
pub(super) struct Frame {
    pub sequence: u64,
    pub pts_100ns: i64,
    pub presented_qpc: i64,
    pub acquired_qpc: i64,
    pub repeated: bool,
}
#[derive(Clone, Deserialize)]
pub(super) struct Entry {
    pub frame: Frame,
    pub duration_100ns: i64,
}
pub(super) struct Selected {
    pub directory: PathBuf,
    pub header: Header,
    pub entry: Entry,
    pub at: i64,
}
pub(super) fn select(
    root: &Path,
    target: i64,
    direction: i8,
    cache: &mut super::index_cache::IndexCache,
    result_anchor: Option<i64>,
    settle_after: Option<i64>,
) -> Result<Selected, String> {
    if !(-1..=1).contains(&direction) {
        return Err("帧方向无效".into());
    }
    if result_anchor.is_some_and(|anchor| direction != 0 || anchor > target) {
        return Err("结果帧区间无效".into());
    }
    if settle_after
        .is_some_and(|start| result_anchor.is_none_or(|anchor| start < anchor) || start > target)
    {
        return Err("结果帧观察区间无效".into());
    }
    let mut dirs = vec![];
    for entry in std::fs::read_dir(root.join("video")).map_err(|_| "此数据包没有视频记录")?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() == 6
            && name.bytes().all(|b| b.is_ascii_digit())
            && entry.file_type().map_err(|e| e.to_string())?.is_dir()
        {
            dirs.push(entry.path());
            if dirs.len() > 256 {
                return Err("视频片段数量超过预算".into());
            }
        }
    }
    dirs.sort();
    for directory in dirs.into_iter().rev() {
        let file =
            std::fs::File::open(directory.join("session.json")).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 65536 {
            return Err("视频元数据超限".into());
        }
        let header: Header = serde_json::from_reader(file).map_err(|e| e.to_string())?;
        if header.qpc_frequency == 0 {
            return Err("视频时钟无效".into());
        }
        if target < header.qpc_origin
            || result_anchor.is_some_and(|anchor| anchor < header.qpc_origin)
        {
            continue;
        }
        let entries = cache.entries(&directory, &header)?;
        let Some(last) = entries.last() else {
            continue;
        };
        let target = if target >= last.end {
            if result_anchor.is_some_and(|anchor| anchor >= header.qpc_origin && anchor < last.end)
            {
                last.end - 1
            } else {
                continue;
            }
        } else {
            target
        };
        let position = match direction {
            -1 => entries.partition_point(|e| e.at < target).checked_sub(1),
            0 => entries.partition_point(|e| e.at <= target).checked_sub(1),
            _ => Some(entries.partition_point(|e| e.at <= target)),
        };
        if let Some(selected) = position.and_then(|i| entries.get(i)) {
            return Ok(Selected {
                directory,
                header,
                entry: selected.entry.clone(),
                at: selected.at,
            });
        }
        return Err("已经到达片段边界".into());
    }
    Err("此时刻没有已保存的视频帧；暂停或停止录制后重试".into())
}
