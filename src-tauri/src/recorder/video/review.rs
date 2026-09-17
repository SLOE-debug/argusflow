//! 视频回看只产生当前请求的PNG，串行解码并限制派生附件容量。
use super::index;
use argusflow_recorder::Attachment;
use serde::Serialize;
use std::{path::PathBuf, sync::OnceLock, time::Duration};
use tokio::sync::Semaphore;
#[cfg(test)]
#[path = "../../../../tests/argusflow-desktop/unit/recorder/video_concurrency.rs"]
mod tests;
#[derive(Serialize)]
pub struct VideoFrame {
    pub image: Attachment,
    pub url: String,
    pub at_qpc: String,
    pub presented_qpc: String,
    pub acquired_qpc: String,
    pub repeated: bool,
    pub segment: String,
    pub sequence: u64,
    pub screen_origin: [i32; 2],
    pub dpi: [u32; 2],
    pub qpc_frequency: u64,
}
#[tauri::command]
pub async fn recorder_video_frame(
    directory: String,
    qpc: String,
    direction: i8,
    result_anchor: Option<String>,
    settle_after: Option<String>,
) -> Result<VideoFrame, String> {
    let result_anchor = result_anchor
        .map(|value| value.parse::<i64>().map_err(|_| "操作时间无效".to_string()))
        .transpose()?;
    let settle_after = settle_after
        .map(|value| value.parse::<i64>().map_err(|_| "观察时间无效".to_string()))
        .transpose()?;
    static DECODING: OnceLock<Semaphore> = OnceLock::new();
    // 正常竞争等待前一帧释放；不把短暂占用变成界面的永久错误。
    let permit = tokio::time::timeout(
        Duration::from_secs(10),
        DECODING.get_or_init(|| Semaphore::new(1)).acquire(),
    )
    .await
    .map_err(|_| "等待视频读取超时，请重新读取")?
    .map_err(|_| "视频读取服务已关闭")?;
    let task = super::worker::submit(
        PathBuf::from(directory),
        qpc,
        direction,
        result_anchor,
        settle_after,
        permit,
    )?;
    // 原生调用不能由取消future中止；实际线程仍持有许可，避免并发失控。
    tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .map_err(|_| "视频读取超过15秒；可重新读取，持续失败请重启应用")?
        .map_err(|e| e.to_string())?
}
pub(super) fn read(
    directory: PathBuf,
    qpc: String,
    direction: i8,
    result_anchor: Option<i64>,
    settle_after: Option<i64>,
    decoder: &mut super::decoder::Decoder,
    indices: &mut super::index_cache::IndexCache,
) -> Result<VideoFrame, String> {
    use base64::Engine;
    argusflow_recorder::load_session(&directory).map_err(|e| e.to_string())?;
    let target = qpc.parse().map_err(|_| "操作时间无效")?;
    let mut selected = index::select(
        &directory,
        target,
        direction,
        indices,
        result_anchor,
        settle_after,
    )?;
    result_anchor
        .map(|anchor| {
            super::analysis::analyze(
                &directory,
                &mut selected,
                anchor,
                settle_after.unwrap_or(anchor),
                target,
                decoder,
                indices,
            )
        })
        .transpose()?;
    let frame = &selected.entry.frame;
    let cache = super::cache::Cache::open(&directory, &selected)?;
    let (image, bytes) = if let Some(cached) = cache.get(&directory)? {
        cached
    } else {
        let decoded = decoder
            .read(&selected.directory.join("screen.mp4"), frame.pts_100ns)
            .map_err(|e| e.to_string())?;
        if [decoded.width, decoded.height] != [selected.header.width, selected.header.height] {
            return Err("视频尺寸与索引不一致".into());
        }
        let bytes = super::image::encode(&decoded)?;
        let image = argusflow_recorder::publish_review_image(
            &directory,
            &bytes,
            [decoded.width, decoded.height],
        )
        .map_err(|e| e.to_string())?;
        cache.put(&directory, &image)?;
        (image, bytes)
    };
    Ok(VideoFrame {
        image,
        url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        ),
        at_qpc: selected.at.to_string(),
        presented_qpc: frame.presented_qpc.to_string(),
        acquired_qpc: frame.acquired_qpc.to_string(),
        repeated: frame.repeated,
        segment: selected
            .directory
            .file_name()
            .ok_or("缺少片段名")?
            .to_string_lossy()
            .into_owned(),
        sequence: frame.sequence,
        screen_origin: selected.header.source.origin,
        dpi: selected.header.source.dpi,
        qpc_frequency: selected.header.qpc_frequency,
    })
}
