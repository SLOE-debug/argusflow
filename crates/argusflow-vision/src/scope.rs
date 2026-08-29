//! VisionRuntime 的窗口与捕获策略作用域状态。

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use argusflow_core::WindowIdentity;
use tokio::sync::Mutex;

use crate::{
    image::CapturedFrame,
    scene::VisualSceneCache,
    source::{CapturePolicy, FrameSubscription},
};

/// 一个视觉作用域由窗口身份和捕获策略共同决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ScopeKey {
    /// 目标窗口身份，包含 HWND 和 PID。
    pub(crate) window: WindowIdentity,
    /// 影响帧语义的捕获策略。
    pub(crate) capture: CapturePolicy,
}

/// 一个作用域内互相隔离的缓存、订阅、差分和噪声状态。
#[derive(Debug)]
pub(crate) struct ScopeState {
    /// 该窗口和捕获策略专属的 scene cache。
    pub(crate) cache: Arc<VisualSceneCache>,
    /// 该作用域复用的捕获订阅。
    pub(crate) subscription: Option<Arc<dyn FrameSubscription>>,
    /// 该作用域上一张稳定帧。
    pub(crate) last_stable_frame: Option<Arc<CapturedFrame>>,
}

impl ScopeState {
    /// 创建一个尚未打开订阅且没有视觉事实的作用域。
    pub(crate) fn new(cache: Arc<VisualSceneCache>) -> Self {
        Self {
            cache,
            subscription: None,
            last_stable_frame: None,
        }
    }
}

/// 作用域注册表中的单个条目；把状态和 cache 放进同一条目，避免并发创建时短暂失配。
#[derive(Debug)]
struct ScopeEntry {
    /// 需要异步锁保护的捕获、差分和订阅状态。
    state: Arc<Mutex<ScopeState>>,
    /// 与 state.cache 相同的同步 cache 引用，供查询和失效路径快速读取。
    cache: Arc<VisualSceneCache>,
    /// 最近一次访问时刻，用于 LRU 和 idle TTL 淘汰。
    last_access: Instant,
}

/// 有界的作用域注册表；长期不访问的窗口状态会被移除。
#[derive(Debug)]
pub(crate) struct ScopeRegistry {
    /// 当前活跃作用域及其独立 cache。
    entries: std::sync::RwLock<HashMap<ScopeKey, ScopeEntry>>,
    /// with_state 构造函数注入的首个测试/宿主 cache。
    bootstrap_cache: std::sync::Mutex<Option<Arc<VisualSceneCache>>>,
    /// 防止多窗口或多种捕获策略无限占用短期内存。
    capacity: usize,
    /// 空闲多久后允许回收作用域及其订阅。
    idle_ttl: Duration,
}

impl ScopeRegistry {
    /// 创建一个带可选首个 cache 的有界注册表。
    pub(crate) fn new(bootstrap_cache: Option<Arc<VisualSceneCache>>) -> Self {
        Self {
            entries: std::sync::RwLock::new(HashMap::new()),
            bootstrap_cache: std::sync::Mutex::new(bootstrap_cache),
            capacity: 8,
            idle_ttl: Duration::from_secs(300),
        }
    }

    /// 获取或创建一个作用域状态，并刷新其 LRU/TTL 时间。
    pub(crate) fn get_or_create(
        &self,
        window: WindowIdentity,
        capture: CapturePolicy,
    ) -> Arc<Mutex<ScopeState>> {
        self.prune_idle();
        let key = ScopeKey { window, capture };
        // 先结束读锁作用域，再由 touch 申请写锁；if-let 的临时值会存活到分支结束，
        // 若直接把 read() 表达式放进条件中，同线程会在持有读锁时永久等待写锁。
        let existing_state = {
            let entries = self
                .entries
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            entries.get(&key).map(|entry| entry.state.clone())
        };
        if let Some(state) = existing_state {
            self.touch(key);
            return state;
        }

        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(state) = entries.get(&key).map(|entry| entry.state.clone()) {
            drop(entries);
            self.touch(key);
            return state;
        }
        let cache = self
            .bootstrap_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
            .unwrap_or_else(|| Arc::new(VisualSceneCache::new()));
        let state = Arc::new(Mutex::new(ScopeState::new(cache.clone())));
        if entries.len() >= self.capacity {
            let stale_key = entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(candidate, _)| *candidate);
            if let Some(stale_key) = stale_key {
                entries.remove(&stale_key);
            }
        }
        entries.insert(
            key,
            ScopeEntry {
                state: state.clone(),
                cache,
                last_access: Instant::now(),
            },
        );
        state
    }

    /// 返回一个作用域的 cache，不受该作用域正在捕获或差分的异步锁影响。
    pub(crate) fn cache_for(
        &self,
        window: WindowIdentity,
        capture: CapturePolicy,
    ) -> Option<Arc<VisualSceneCache>> {
        self.prune_idle();
        let key = ScopeKey { window, capture };
        let cache = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&key)
            .map(|entry| entry.cache.clone());
        if cache.is_some() {
            self.touch(key);
        }
        cache
    }

    /// 返回所有 cache，供同步诊断和全局失效操作遍历。
    pub(crate) fn cache_snapshot(&self) -> Vec<Arc<VisualSceneCache>> {
        self.prune_idle();
        let mut caches = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .map(|entry| entry.cache.clone())
            .collect::<Vec<_>>();
        if let Some(bootstrap) = self
            .bootstrap_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .cloned()
        {
            caches.push(bootstrap);
        }
        caches
    }

    /// 移除窗口身份已失效或捕获订阅已关闭的作用域，避免下一次继续复用旧 HWND。
    pub(crate) fn remove(&self, window: WindowIdentity, capture: CapturePolicy) {
        self.entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&ScopeKey { window, capture });
    }

    /// 按 idle TTL 移除不再访问的作用域；活跃调用持有的 Arc 状态仍可安全完成当前动作。
    fn prune_idle(&self) {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.retain(|_, entry| entry.last_access.elapsed() <= self.idle_ttl);
    }

    /// 更新作用域最近访问时间。
    fn touch(&self, key: ScopeKey) {
        if let Some(entry) = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(&key)
        {
            entry.last_access = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };

    use super::*;

    /// 已存在作用域的命中路径必须先释放读锁，才能刷新其 LRU 写状态。
    #[test]
    fn existing_scope_is_reused_without_lock_upgrade_deadlock() {
        let registry = Arc::new(ScopeRegistry::new(None));
        let window = WindowIdentity {
            handle: 41,
            process_id: 42,
        };
        let capture = CapturePolicy::default();
        let expected = registry.get_or_create(window, capture);
        let worker_registry = registry.clone();
        let (sender, receiver) = mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let actual = worker_registry.get_or_create(window, capture);
            let _ = sender.send(actual);
        });

        let actual = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("existing scope lookup must not block while refreshing LRU state");
        assert!(Arc::ptr_eq(&expected, &actual));
    }
}
