//! 单锁保护元数据和版本索引；锁内不执行原生调用。
use super::{ChangeKind, ChangeRecord};
use crate::CaptureConfig;
use argusflow_capture_contracts::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

pub(crate) struct SourceEntry {
    pub info: SourceInfo,
    pub history: VecDeque<Arc<Snapshot>>,
    pub watermark: ClockTime,
    pub log_floor: ClockTime,
}
pub(crate) struct State {
    pub session: u128,
    pub sources: BTreeMap<SourceId, SourceEntry>,
    pub records: VecDeque<ChangeRecord>,
    pub sequence: u64,
    pub bytes: usize,
    pub failure: Option<CaptureError>,
    pub stopped: bool,
}
impl State {
    pub fn new(session: u128) -> Self {
        Self {
            session,
            sources: BTreeMap::new(),
            records: VecDeque::new(),
            sequence: 0,
            bytes: 0,
            failure: None,
            stopped: false,
        }
    }
    pub fn apply(&mut self, event: BackendEvent, now: ClockTime, config: &CaptureConfig) {
        if self.failure.is_some() {
            return;
        }
        if let Err(error) = self.validate_event(&event) {
            self.failure = Some(error);
            return;
        }
        let record = match event {
            BackendEvent::Source(info) => {
                if let Some(entry) = self.sources.get_mut(&info.id) {
                    if entry.info.generation != info.generation || info.state != SourceState::Ready
                    {
                        entry.history.clear();
                        entry.watermark = ClockTime(0);
                    }
                    entry.info = info.clone();
                } else {
                    self.sources.insert(
                        info.id,
                        SourceEntry {
                            info: info.clone(),
                            history: VecDeque::new(),
                            watermark: ClockTime(0),
                            log_floor: ClockTime(0),
                        },
                    );
                }
                Some((info.id, now, ChangeKind::Source(info)))
            }
            BackendEvent::Baseline(snapshot) => {
                if let Some(entry) = self.sources.get_mut(&snapshot.version.source) {
                    entry.history.clear();
                    entry.info.generation = snapshot.version.generation;
                    entry.info.state = SourceState::Ready;
                    entry.history.push_back(snapshot.clone());
                }
                Some((
                    snapshot.version.source,
                    snapshot.timing.acquired,
                    ChangeKind::Baseline(snapshot.version),
                ))
            }
            BackendEvent::Changed { snapshot, changes } => {
                if changes.changed_pixels == 0 {
                    return;
                }
                if let Some(entry) = self.sources.get_mut(&snapshot.version.source) {
                    entry.history.push_back(snapshot.clone());
                }
                Some((
                    snapshot.version.source,
                    snapshot
                        .timing
                        .presented
                        .unwrap_or(snapshot.timing.acquired),
                    ChangeKind::Pixels {
                        version: snapshot.version,
                        changes,
                    },
                ))
            }
            BackendEvent::Watermark {
                source,
                generation,
                through,
            } => {
                if let Some(entry) = self.sources.get_mut(&source)
                    && entry.info.generation == generation
                    && entry.info.state == SourceState::Ready
                {
                    entry.watermark = entry.watermark.max(through);
                }
                None
            }
            BackendEvent::Gap(gap) => {
                if matches!(
                    gap.reason,
                    GapReason::Capacity
                        | GapReason::DeviceReset
                        | GapReason::TopologyChanged
                        | GapReason::Unavailable
                        | GapReason::Protected
                ) && let Some(entry) = self.sources.get_mut(&gap.source)
                {
                    // 压力时先释放服务持有的普通历史；外部固定租约依然计费。
                    entry.history.clear();
                    entry.watermark = ClockTime(0);
                }
                Some((gap.source, gap.through, ChangeKind::Gap(gap)))
            }
        };
        if let Some((source, time, kind)) = record {
            self.sequence += 1;
            let record = ChangeRecord {
                sequence: self.sequence,
                source,
                time,
                kind,
            };
            let bytes = record.byte_len();
            while self.bytes.saturating_add(bytes) > config.metadata_bytes {
                let Some(old) = self.records.pop_front() else {
                    break;
                };
                self.bytes -= old.byte_len();
                if let Some(entry) = self.sources.get_mut(&old.source) {
                    entry.log_floor = entry.log_floor.max(old.time);
                }
            }
            if bytes <= config.metadata_bytes {
                self.bytes += bytes;
                self.records.push_back(record);
            } else if let Some(entry) = self.sources.get_mut(&source) {
                entry.log_floor = entry.log_floor.max(time);
            }
        }
        self.expire(now, config);
    }
    pub fn expire(&mut self, now: ClockTime, config: &CaptureConfig) {
        let cutoff = ClockTime(now.0.saturating_sub(config.history.as_nanos() as u64));
        for entry in self.sources.values_mut() {
            // 留下窗口起点之前的最近一版，静态桌面不会因呈现时间较老而丢失基线。
            while entry.history.len() > 1
                && (entry.history.len() > config.history_versions
                    || entry.history.get(1).is_some_and(|next| {
                        next.timing.presented.unwrap_or(next.timing.acquired) < cutoff
                    }))
            {
                entry.history.pop_front();
            }
        }
    }
}
