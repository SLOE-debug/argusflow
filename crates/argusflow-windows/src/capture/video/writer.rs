//! 消费有界GPU帧并持续写入视频及逐帧索引。
use super::{
    encoder::Encoder,
    journal::{Journal, Message, PendingFrame},
    model::Result,
    sample,
};
use std::{
    path::Path,
    sync::mpsc::{Receiver, SyncSender},
};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;

pub(super) fn encode(
    encoder: &mut Encoder,
    receiver: &Receiver<Message>,
    returned: SyncSender<ID3D11Texture2D>,
    directory: &Path,
    fps: u32,
) -> Result<()> {
    let callback = sample::recycler(returned);
    let mut journal = Journal::new(&directory.join("frames.jsonl"))?;
    let mut pending: Option<PendingFrame> = None;
    for message in receiver {
        let end = match &message {
            Message::Frame(frame) => frame.entry.pts_100ns,
            Message::End(end) => *end,
        };
        if let Some(frame) = pending.take() {
            // 结束恰好落在末帧的同一毫秒时，保留一个最小媒体时间单位。
            let end = if matches!(message, Message::End(_)) {
                end.max(frame.entry.pts_100ns + 10_000)
            } else {
                end
            };
            let duration = super::timing::duration(frame.entry.pts_100ns, end)?;
            encoder.write(&frame, duration, &callback)?;
            journal.write(&serde_json::json!({"frame":frame.entry,"duration_100ns":duration,"status":"submitted"}))?;
        }
        match message {
            Message::Frame(frame) => pending = Some(frame),
            Message::End(_) => break,
        }
    }
    if let Some(frame) = pending {
        encoder.write(&frame, 10_000_000 / i64::from(fps), &callback)?;
        journal.write(&serde_json::json!({"frame":frame.entry,"duration_100ns":10_000_000 / i64::from(fps),"status":"submitted"}))?;
    }
    encoder.finish()?;
    journal.sync()?;
    // 同步已完成的视频，不把仅接受样本等同于文件落盘。
    std::fs::OpenOptions::new()
        .write(true)
        .open(directory.join("screen.mp4"))?
        .sync_all()?;
    Ok(())
}
