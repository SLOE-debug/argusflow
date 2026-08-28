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
    stability::TemporalNoiseMask,
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
    /// 该作用域的时序噪声判定状态。
    pub(crate) temporal_noise: TemporalNoiseMask,
}

impl ScopeState {
    /// 创建一个尚未打开订阅且没有视觉事实的作用域。
    pub(crate) fn new(cache: Arc<VisualSceneCache>) -> Self {
        Self {
            cache,
            subscription: None,
            last_stable_frame: None,
            temporal_noise: TemporalNoiseMask::default(),
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
        if let Some(state) = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&key)
            .map(|entry| entry.state.clone())
        {
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
