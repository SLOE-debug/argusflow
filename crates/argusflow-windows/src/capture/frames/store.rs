//! 帧缓存只保存不可变图像和来源水位，读取者不会锁住原生采集。
use argusflow_capture_contracts::*;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Default)]
pub(super) struct Store {
    pub sources: BTreeMap<SourceId, FrameHistory>,
    pub failure: Option<CaptureError>,
}
impl Store {
    pub fn source(&mut self, source: SourceInfo) {
        let entry = self
            .sources
            .entry(source.id)
            .or_insert_with(|| FrameHistory {
                source: source.clone(),
                checked: ClockTime(0),
                frames: Vec::new(),
            });
        if source.generation != entry.source.generation || source.state != SourceState::Ready {
            entry.frames.clear();
            entry.checked = ClockTime(0);
        }
        entry.source = source;
    }
    pub fn publish(&mut self, frame: DesktopFrame, checked: ClockTime, limit: usize) {
        self.source(frame.source.clone());
        if let Some(entry) = self.sources.get_mut(&frame.source.id) {
            entry.frames.push(Arc::new(frame));
            if entry.frames.len() > limit {
                entry.frames.remove(0);
            }
            entry.checked = checked;
        }
    }
    pub fn checked(&mut self, source: SourceId, checked: ClockTime) {
        if let Some(entry) = self.sources.get_mut(&source)
            && !entry.frames.is_empty()
        {
            entry.checked = checked;
        }
    }
    /// 先腾出历史缓存；正在编码的已固定图像继续计费，不覆盖其内容。
    pub fn make_room(&mut self, budget: &ByteBudget, bytes: usize) {
        while budget.used().saturating_add(bytes) > budget.limit() {
            let oldest = self
                .sources
                .iter()
                .filter(|(_, entry)| entry.frames.len() > 1)
                .min_by_key(|(_, entry)| entry.frames[0].timing.acquired)
                .map(|(id, _)| *id);
            let Some(id) = oldest else {
                break;
            };
            if let Some(entry) = self.sources.get_mut(&id) {
                entry.frames.remove(0);
            }
        }
    }
}
