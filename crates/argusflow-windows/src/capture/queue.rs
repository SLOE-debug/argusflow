//! 有限原生事件桥；压力只丢弃未被消费的历史，显式报告每个受影响来源。
use argusflow_capture_contracts::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
struct State {
    events: VecDeque<BackendEvent>,
    bytes: usize,
    gaps: BTreeMap<SourceId, Gap>,
    sources: BTreeMap<SourceId, SourceInfo>,
    latest: BTreeMap<SourceId, std::sync::Arc<Snapshot>>,
    overflow: bool,
    failure: Option<CaptureError>,
}
pub(super) struct EventQueue {
    state: Mutex<State>,
    limit: usize,
    gaps: AtomicU64,
}
impl EventQueue {
    pub fn new(limit: usize) -> Self {
        Self {
            state: Mutex::new(State {
                events: VecDeque::new(),
                bytes: 0,
                gaps: BTreeMap::new(),
                sources: BTreeMap::new(),
                latest: BTreeMap::new(),
                overflow: false,
                failure: None,
            }),
            limit,
            gaps: AtomicU64::new(0),
        }
    }
    pub fn push(&self, event: BackendEvent, now: ClockTime) {
        if matches!(event, BackendEvent::Gap(_)) {
            self.gaps.fetch_add(1, Ordering::Relaxed);
        }
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.failure.is_some() {
            return;
        }
        match &event {
            BackendEvent::Source(info) => {
                if !state.sources.contains_key(&info.id) && state.sources.len() >= 512 {
                    state.failure = Some(CaptureError::new(
                        argusflow_core::FailureKind::ResourceLimit,
                        "capture_sources",
                        "会话累计来源超过 512 个",
                    ));
                    return;
                }
                state.sources.insert(info.id, info.clone());
                if info.state != SourceState::Ready {
                    state.latest.remove(&info.id);
                }
            }
            BackendEvent::Baseline(frame)
            | BackendEvent::Changed {
                snapshot: frame, ..
            } => {
                state.latest.insert(frame.version.source, frame.clone());
            }
            _ => {}
        }
        let bytes = match &event {
            BackendEvent::Changed { changes, .. } => {
                256 + changes.regions.capacity() * std::mem::size_of::<PixelRect>()
            }
            BackendEvent::Source(info) => 256 + info.name.capacity(),
            _ => 256,
        };
        if state.bytes.saturating_add(bytes) > self.limit {
            state.events.clear();
            state.bytes = 0;
            state.overflow = true;
            let ids: Vec<_> = state.sources.keys().copied().collect();
            for source in ids {
                state
                    .gaps
                    .entry(source)
                    .or_insert(Gap {
                        source,
                        from: ClockTime(0),
                        through: now,
                        reason: GapReason::Capacity,
                    })
                    .through = now;
            }
        }
        if !state.overflow {
            state.bytes += bytes;
            state.events.push_back(event);
        } else {
            for gap in state.gaps.values_mut() {
                gap.through = now;
            }
        }
    }
    pub fn drain(&self) -> Vec<BackendEvent> {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.overflow {
            self.gaps
                .fetch_add(state.gaps.len() as u64, Ordering::Relaxed);
            state.overflow = false;
            state.bytes = 0;
            let mut events: Vec<_> = state
                .sources
                .values()
                .cloned()
                .map(BackendEvent::Source)
                .collect();
            events.extend(
                std::mem::take(&mut state.gaps)
                    .into_values()
                    .map(BackendEvent::Gap),
            );
            events.extend(state.latest.values().cloned().map(BackendEvent::Baseline));
            return events;
        }
        state.bytes = 0;
        state.events.drain(..).collect()
    }
    pub fn gap_count(&self) -> u64 {
        self.gaps.load(Ordering::Relaxed)
    }
    pub fn failure(&self) -> Option<CaptureError> {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .failure
            .clone()
    }
    pub fn resynchronize(&self, now: ClockTime) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.events.clear();
        state.bytes = 0;
        state.overflow = true;
        let ids: Vec<_> = state.sources.keys().copied().collect();
        for source in ids {
            state.gaps.insert(
                source,
                Gap {
                    source,
                    from: ClockTime(0),
                    through: now,
                    reason: GapReason::ConsumerLagged,
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-windows/unit/capture/queue.rs"]
mod tests;
