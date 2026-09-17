//! 回看总览只传递时间与键鼠标记，不读取或编码像素。
use super::{index::Header, index_cache::IndexCache};
use argusflow_input_contracts::InputKind;
use argusflow_recorder::{Cursor, RecordData, load_session, read_page};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct VideoTimeline {
    pub origin_qpc: String,
    pub frequency: u64,
    pub duration_ms: f64,
    pub segments: Vec<Segment>,
    pub markers: Vec<Marker>,
    pub tail: Option<String>,
}
#[derive(Serialize)]
pub struct Segment {
    pub start_ms: f64,
    pub end_ms: f64,
}
#[derive(Serialize)]
pub struct Marker {
    pub id: u64,
    pub time_ms: f64,
    pub input: MarkerInput,
}
/// 双轨道只标记输入，窗口通知不会被冒充为鼠标事件。
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarkerInput {
    Key {
        vk: u32,
    },
    Button {
        button: argusflow_input_contracts::Button,
    },
    Wheel {
        horizontal: bool,
        delta: i16,
    },
}

#[tauri::command]
pub async fn recorder_video_timeline(directory: String) -> Result<VideoTimeline, String> {
    tokio::task::spawn_blocking(move || read(Path::new(&directory)))
        .await
        .map_err(|e| e.to_string())?
}

fn read(root: &Path) -> Result<VideoTimeline, String> {
    let session = load_session(root).map_err(|e| e.to_string())?;
    let millis = |qpc: i64| {
        (i128::from(qpc) - i128::from(session.qpc_origin)) as f64 * 1000.0
            / session.qpc_frequency as f64
    };
    let mut result = VideoTimeline {
        origin_qpc: session.qpc_origin.to_string(),
        frequency: session.qpc_frequency,
        duration_ms: 0.0,
        segments: vec![],
        markers: vec![],
        tail: None,
    };
    let mut cache = IndexCache::default();
    for entry in std::fs::read_dir(root.join("video")).map_err(|_| "此数据包没有视频记录")?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.len() != 6
            || !name.bytes().all(|b| b.is_ascii_digit())
            || !entry.file_type().map_err(|e| e.to_string())?.is_dir()
        {
            continue;
        }
        if result.segments.len() >= 256 {
            return Err("视频片段数量超过预算".into());
        }
        let directory = entry.path();
        let file =
            std::fs::File::open(directory.join("session.json")).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > 65536 {
            return Err("视频元数据超限".into());
        }
        let header: Header = serde_json::from_reader(file).map_err(|e| e.to_string())?;
        if header.qpc_frequency != session.qpc_frequency {
            return Err("视频与会话时钟不一致".into());
        }
        let entries = cache.entries(&directory, &header)?;
        if let (Some(first), Some(last)) = (entries.first(), entries.last()) {
            result.segments.push(Segment {
                start_ms: millis(first.at),
                end_ms: millis(last.end),
            });
            result.duration_ms = result.duration_ms.max(millis(last.end));
        }
    }
    result
        .segments
        .sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    let mut cursor = Cursor::default();
    loop {
        let page = read_page(root, cursor, 256).map_err(|e| e.to_string())?;
        cursor = page.cursor;
        for record in page.records {
            if let RecordData::Raw(raw) = record.data {
                let input = match raw.kind {
                    InputKind::Key { vk, down: true, .. } => MarkerInput::Key { vk },
                    InputKind::Button { button, down: true } => MarkerInput::Button { button },
                    InputKind::Wheel { horizontal, delta } => {
                        MarkerInput::Wheel { horizontal, delta }
                    }
                    _ => continue,
                };
                if result.markers.len() >= 100_000 {
                    return Err("时间线输入标记超过10万条预算".into());
                }
                result.markers.push(Marker {
                    id: record.id,
                    time_ms: millis(raw.qpc),
                    input,
                });
            }
        }
        if page.end || page.tail.is_some() {
            result.tail = page.tail;
            break;
        }
    }
    result
        .markers
        .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
    Ok(result)
}
