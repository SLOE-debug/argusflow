//! WindowScene cache、脏区失效和物理输入前复验。

use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use argusflow_core::WindowIdentity;

use super::{SceneRefreshPolicy, VisionRuntime};
use crate::{
    CapturePolicy, CapturedFrame, DiffConfig, FrameSubscription, PhysicalRect, TopologyGeneration,
    VisionError, VisualScene, compute_dirty_map,
    scene::{CacheLookup, CacheMissReason},
};

impl VisionRuntime {
    /// 返回全部窗口 cache 中 Scene ID 最大的一份只读 Scene。
    pub fn cached_scene(&self) -> Option<Arc<VisualScene>> {
        self.scopes
            .cache_snapshot()
            .into_iter()
            .filter_map(|cache| cache.current())
            .max_by_key(|scene| scene.scene_id)
    }

    /// 按窗口和 generation 查询 cache。
    pub fn lookup_cache(&self, window: WindowIdentity, policy: &SceneRefreshPolicy) -> CacheLookup {
        let cache = self.scopes.cache_for(window, policy.capture).or_else(|| {
            self.scopes.get_or_create(window, policy.capture);
            self.scopes.cache_for(window, policy.capture)
        });
        let Some(cache) = cache else {
            return CacheLookup::Miss(CacheMissReason::Empty);
        };
        cache.lookup(window, policy.topology_generation, policy.max_age, None)
    }

    /// 消费一张新捕获帧并更新 cache dirty 状态，不执行 OCR。
    pub async fn revalidate_cache(
        &self,
        window: WindowIdentity,
        timeout: Duration,
    ) -> Result<TopologyGeneration, VisionError> {
        let capture = CapturePolicy::default();
        let scope = self.scopes.get_or_create(window, capture);
        let subscription = self.subscription(&scope, window, capture).await?;
        if subscription.window() != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(subscription.window()),
            });
        }
        let frame = subscription.next(timeout).await?;
        if frame.window != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(frame.window),
            });
        }
        self.update_cache_invalidation(&scope, window, &frame, DiffConfig::default())
            .await?;
        Ok(frame.topology_generation)
    }

    /// 在输入提交点复验物化目标绑定的 Scene、Frame、Topology 和 ROI。
    pub async fn validate_materialized_target(
        &self,
        window: WindowIdentity,
        scene_id: u64,
        frame_id: u64,
        topology_generation: u64,
        target_region: PhysicalRect,
    ) -> Result<(), VisionError> {
        let current_topology = self
            .revalidate_cache(window, Duration::from_millis(75))
            .await?;
        let expected_topology = TopologyGeneration::new(topology_generation);
        if !expected_topology.is_unknown() && current_topology != expected_topology {
            return Err(VisionError::SceneStale);
        }
        let capture = CapturePolicy::default();
        let scope = self.scopes.get_or_create(window, capture);
        let (last_frame, cache) = {
            let state = scope.lock().await;
            (state.last_stable_frame.clone(), state.cache.clone())
        };
        let Some(last_frame) = last_frame else {
            return Err(VisionError::SceneStale);
        };
        if last_frame.window != window
            || (!expected_topology.is_unknown()
                && last_frame.topology_generation != expected_topology)
            || last_frame.frame_id.get() < frame_id
            || cache.is_region_dirty(target_region)
        {
            return Err(VisionError::SceneStale);
        }
        let Some(scene) = cache.current() else {
            return Err(VisionError::SceneStale);
        };
        if scene.window != window
            || scene.scene_id.get() != scene_id
            || scene.frame_id.get() != frame_id
            || (!expected_topology.is_unknown() && scene.topology_generation != expected_topology)
            || scene.frame_id.get() > last_frame.frame_id.get()
        {
            return Err(VisionError::SceneStale);
        }
        Ok(())
    }

    /// 失效与 dirty map 相交的全部窗口 cache 区域。
    pub fn invalidate(&self, dirty: &crate::DirtyMap) {
        for cache in self.scopes.cache_snapshot() {
            cache.invalidate(dirty);
        }
    }

    /// 将当前稳定帧与上一份稳定帧比较，维护 WindowScene 的 ROI 失效边界。
    pub(super) async fn update_cache_invalidation(
        &self,
        scope: &Arc<tokio::sync::Mutex<crate::scope::ScopeState>>,
        window: WindowIdentity,
        frame: &Arc<CapturedFrame>,
        diff_config: DiffConfig,
    ) -> Result<Option<crate::DirtyMap>, VisionError> {
        let mut state = scope.lock().await;
        let previous = state.last_stable_frame.clone();
        let dirty = if let Some(previous_frame) = previous {
            if previous_frame.window != window
                || previous_frame.topology_generation != frame.topology_generation
            {
                state.cache.clear();
                self.metrics.record_diff(1.0);
                None
            } else {
                let dirty =
                    compute_dirty_map(Some(previous_frame.as_ref()), frame.as_ref(), diff_config)?;
                state.cache.invalidate(&dirty);
                self.metrics.record_diff(dirty.changed_area_ratio);
                Some(dirty)
            }
        } else {
            state.cache.clear();
            self.metrics.record_diff(1.0);
            None
        };
        state.last_stable_frame = Some(frame.clone());
        Ok(dirty)
    }

    /// 在不跨 await 持锁的前提下复用同一窗口的 capture subscription。
    pub(super) async fn subscription(
        &self,
        scope: &Arc<tokio::sync::Mutex<crate::scope::ScopeState>>,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        if let Some(subscription) = scope.lock().await.subscription.clone() {
            return Ok(subscription);
        }
        let opened = match self.capture.open(window, policy).await {
            Ok(subscription) => subscription,
            Err(error) => {
                self.capture_ready.store(false, Ordering::Relaxed);
                return Err(error);
            }
        };
        self.capture_ready.store(true, Ordering::Relaxed);
        let mut state = scope.lock().await;
        if let Some(subscription) = state.subscription.clone() {
            Ok(subscription)
        } else {
            state.subscription = Some(opened.clone());
            Ok(opened)
        }
    }
}
