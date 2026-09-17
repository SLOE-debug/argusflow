//! 顺序扫描有限操作区间，只解码分析小图；最终原图由回看入口按选中PTS解码。
use super::super::{decoder::Decoder, index::Selected, index_cache::IndexCache};
use super::{cache::Cache, model::Analysis, selection::Selection};
use std::path::Path;
pub(in crate::recorder::video) fn analyze(
    root: &Path,
    selected: &mut Selected,
    anchor: i64,
    earliest: i64,
    target: i64,
    decoder: &mut Decoder,
    indices: &mut IndexCache,
) -> Result<Analysis, String> {
    let video = selected.directory.join("screen.mp4");
    let entries = indices.entries(&selected.directory, &selected.header)?;
    let start = entries[..entries.partition_point(|entry| entry.at <= anchor)]
        .iter()
        .rposition(|entry| entry.entry.frame.presented_qpc < anchor)
        .ok_or("操作前缺少视频基准帧")?;
    let stop = entries.partition_point(|entry| entry.at <= selected.at);
    if stop - start > 121 {
        return Err("操作分析区间超过121帧预算".into());
    }
    let end = entries[stop - 1].end.min(target);
    let cache = Cache::open(root, &video, anchor, target, earliest)?;
    let decision = if let Some(decision) = cache.get() {
        decision
    } else {
        let baseline = &entries[start];
        let thumb = decoder.thumbnail(&video, baseline.entry.frame.pts_100ns)?;
        let mut selection = Selection::new(thumb, baseline.entry.frame.sequence, baseline.at);
        let mut settled = false;
        for entry in &entries[start + 1..stop] {
            if entry.entry.frame.repeated {
                continue;
            }
            let thumbnail = decoder.thumbnail(&video, entry.entry.frame.pts_100ns)?;
            if selection.push(
                thumbnail,
                entry.entry.frame.sequence,
                entry.at,
                entry.end.min(end),
                earliest,
                selected.header.qpc_frequency,
            )? {
                settled = true;
                break;
            }
        }
        let decision = selection.decision(settled, [selected.header.width, selected.header.height]);
        cache.put(root, decision.clone())?;
        decision
    };
    let chosen = entries[start..stop]
        .iter()
        .find(|entry| entry.entry.frame.sequence == decision.sequence)
        .ok_or("分析结果与视频索引不一致")?;
    selected.entry = chosen.entry.clone();
    selected.at = chosen.at;
    Ok(decision.analysis)
}
