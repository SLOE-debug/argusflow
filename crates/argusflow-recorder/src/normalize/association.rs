//! 有界时间索引只声明观察期间的输入共现，不推断因果。
use crate::{Record, RecordData, Relation, Session};
use std::collections::VecDeque;

pub(super) struct TemporalIndex {
    inputs: VecDeque<(u64, u64)>,
    origin: i64,
    frequency: u64,
    evicted_through: Option<u64>,
}

#[cfg(test)]
#[path = "../../../../tests/argusflow-recorder/unit/association.rs"]
mod tests;
impl TemporalIndex {
    pub fn new(session: &Session) -> Self {
        Self {
            inputs: VecDeque::new(),
            origin: session.qpc_origin,
            frequency: session.qpc_frequency,
            evicted_through: None,
        }
    }
    pub fn push(&mut self, id: u64, qpc: i64) {
        let ns = (qpc.saturating_sub(self.origin).max(0) as u128 * 1_000_000_000
            / u128::from(self.frequency))
        .min(u128::from(u64::MAX)) as u64;
        self.inputs.push_back((id, ns));
        if self.inputs.len() > 8192 {
            self.evicted_through = self.inputs.pop_front().map(|(_, time)| time);
        }
    }
    pub fn associate(&self, record: &Record) -> Option<RecordData> {
        let RecordData::Visual(visual) = &record.data else {
            return None;
        };
        if matches!(
            visual.relation,
            Relation::Before | Relation::Input | Relation::Response
        ) {
            return None;
        }
        let mut matching = self
            .inputs
            .iter()
            .filter(|(_, ns)| *ns >= visual.from_ns && *ns <= visual.through_ns);
        let mut records = vec![record.id];
        records.extend(matching.by_ref().take(2047).map(|(id, _)| *id));
        let truncated = matching.next().is_some()
            || self.evicted_through.is_some_and(|ns| ns >= visual.from_ns);
        Some(RecordData::Association {
            records,
            relation: format!(
                "视觉观察区间内已提交的输入（包含后续操作）；仅时间共现，不证明因果；时间索引截断={truncated}"
            ),
        })
    }
    pub fn reset(&mut self) {
        self.inputs.clear();
        self.evicted_through = None;
    }
}
