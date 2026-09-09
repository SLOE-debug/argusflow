//! 缓存所属实例固定引擎配置；同区域任务共享结果，最后等待者退出时取消。
use super::model::{SampledOcrError, SampledOcrResult};
use argusflow_capture_contracts::{ContentToken, PixelRect, SourceId};
use argusflow_core::Operation;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::watch;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Key {
    pub source: SourceId,
    pub region: PixelRect,
}
#[derive(Clone)]
pub(super) struct Cached {
    pub token: ContentToken,
    pub output: SampledOcrResult,
}
pub(super) struct Flight {
    pub operation: Operation,
    pub users: AtomicUsize,
    pub result: watch::Sender<Option<Result<SampledOcrResult, SampledOcrError>>>,
}
pub(super) struct Waiter(pub Arc<Flight>);
impl Drop for Waiter {
    fn drop(&mut self) {
        if self.0.users.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.operation.cancel();
        }
    }
}
pub(super) struct Entry {
    pub cached: Option<Cached>,
    pub flight: Option<Arc<Flight>>,
    pub used: u64,
}
pub(super) struct Cache {
    pub entries: HashMap<Key, Entry>,
    pub tick: u64,
    pub capacity: usize,
}
impl Cache {
    pub fn reserve(&mut self, key: Key) -> bool {
        if self.entries.contains_key(&key) {
            return true;
        }
        if self.entries.len() >= self.capacity {
            let victim = self
                .entries
                .iter()
                .filter(|(_, entry)| entry.flight.is_none())
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key);
            let Some(victim) = victim else {
                return false;
            };
            self.entries.remove(&victim);
        }
        true
    }
}
