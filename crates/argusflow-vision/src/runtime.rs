//! 共享 VisionRuntime：捕获、稳定帧、worker、scene cache 和 metrics 只装配一次。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use argusflow_core::{NormalizedRect, RunTraceContext, WindowIdentity};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    diff::{DiffConfig, compute_dirty_map},
    error::VisionError,
    frame::{PhysicalRect, TopologyGeneration},
    image::CapturedFrame,
    index::VisualSceneSnapshot,
    metrics::VisionMetrics,
    ocr::OcrProfile,
    projection::ProjectionOptions,
    scene::{CacheLookup, VisualScene, VisualSceneBuilder, VisualSceneCache},
    source::{CapturePolicy, FrameSubscription, WindowFrameSource},
    stability::StabilityConfig,
    worker::{VisionWorkerClient, WorkerHealth},
};

mod scene_refresh;

/// 获取新 scene 时的刷新策略。
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRefreshPolicy {
    /// 是否跳过 cache freshness 检查并重新取稳定帧。
    pub force_refresh: bool,
    /// 是否忽略查询 ROI 和 dirty map，强制整窗 OCR。
    pub force_full_ocr: bool,
    /// cache 允许的最大年龄。
    pub max_age: Duration,
    /// 捕获流策略。
    pub capture: CapturePolicy,
    /// 稳定帧门控策略。
    pub stability: StabilityConfig,
    /// stable gate 使用的低分辨率差分策略。
    pub diff: DiffConfig,
    /// OCR 模型 profile。
    pub ocr: OcrProfile,
    /// 单次 OCR 请求自己的截止时间；不与 stable gate 的观察预算混用。
    pub ocr_timeout: Duration,
    /// 只刷新与该 ROI 相交的内容；为空表示当前 viewport。
    pub query_region: Option<PhysicalRect>,
    /// 以当前视觉 viewport 百分比表达的查询区域；由 runtime 映射为物理 ROI。
    pub normalized_query_region: Option<NormalizedRect>,
    /// 当前调用方已确认的拓扑代数。
    pub topology_generation: TopologyGeneration,
    /// Compact/Spatial 文本设置。
    pub projection: ProjectionOptions,
}

impl SceneRefreshPolicy {
    /// 创建 cache-first tiny 刷新策略。
    pub fn tiny() -> Self {
        Self {
            force_refresh: false,
            force_full_ocr: false,
            max_age: Duration::from_millis(500),
            capture: CapturePolicy::default(),
            stability: StabilityConfig::default(),
            diff: DiffConfig::default(),
            ocr: OcrProfile::tiny(),
            ocr_timeout: Duration::from_secs(3),
            query_region: None,
            normalized_query_region: None,
            topology_generation: TopologyGeneration::new(0),
            projection: ProjectionOptions::default(),
        }
    }

    /// 创建桌面 GUI 默认使用的 small ROI 刷新策略。
    pub fn small() -> Self {
        Self {
            max_age: Duration::from_millis(400),
            ocr: OcrProfile::small(),
            ocr_timeout: Duration::from_secs(6),
            ..Self::tiny()
        }
    }

    /// 创建关键验证使用的 medium 强制刷新策略。
    pub fn medium() -> Self {
        Self {
            force_refresh: true,
            max_age: Duration::from_millis(250),
            ocr: OcrProfile::medium(),
            ocr_timeout: Duration::from_secs(10),
            ..Self::small()
        }
    }
}

impl Default for SceneRefreshPolicy {
    fn default() -> Self {
        Self::tiny()
    }
}

/// VisionRuntime 的可观察健康摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionHealth {
    /// 捕获源已装配且最近一次打开尝试未失败。
    pub capture_ready: bool,
    /// worker 当前是否允许 OCR。
    pub worker_ready: bool,
    /// worker health 的完整版本/模型信息。
    pub worker: WorkerHealth,
}

/// current viewport scene 的稳定异步服务契约。
#[async_trait]
pub trait VisualSceneService: Send + Sync {
    /// 按窗口身份和刷新策略取得一份当前稳定 scene。
    async fn current_scene(
        &self,
        window: WindowIdentity,
        policy: SceneRefreshPolicy,
    ) -> Result<Arc<VisualScene>, VisionError>;
}

/// 跨后端共享的视觉运行时。
#[derive(Debug)]
pub struct VisionRuntime {
    /// 按 HWND 打开的单一捕获源。
    capture: Arc<dyn WindowFrameSource>,
    /// 共享的 Python/Named Pipe worker client。
    worker: Arc<VisionWorkerClient>,
    /// 按窗口和捕获策略隔离的短期视觉状态。
    scopes: crate::scope::ScopeRegistry,
    /// scene builder 的单调 ID 状态。
    scene_builder: Mutex<VisualSceneBuilder>,
    /// 运行指标。
    metrics: Arc<VisionMetrics>,
    /// 捕获源健康状态。
    capture_ready: AtomicBool,
    /// 可选宿主 artifact sink；失败语义由 sink 自己 best-effort 吞吐。
    trace_sink: Option<Arc<dyn crate::VisionTraceSink>>,
}

impl VisionRuntime {
    /// 使用共享 capture 和 worker 创建 runtime。
    pub fn new(capture: Arc<dyn WindowFrameSource>, worker: Arc<VisionWorkerClient>) -> Self {
        Self {
            capture,
            worker,
            scopes: crate::scope::ScopeRegistry::new(None),
            scene_builder: Mutex::new(VisualSceneBuilder::new()),
            metrics: Arc::new(VisionMetrics::default()),
            // capture source 已由宿主装配；首次 execute 仍会执行真实 HWND/PID 校验。
            capture_ready: AtomicBool::new(true),
            trace_sink: None,
        }
    }

    /// 创建使用指定 cache/metrics 的 runtime，便于宿主统一观察数据。
    pub fn with_state(
        capture: Arc<dyn WindowFrameSource>,
        worker: Arc<VisionWorkerClient>,
        cache: Arc<VisualSceneCache>,
        metrics: Arc<VisionMetrics>,
    ) -> Self {
        Self {
            capture,
            worker,
            scopes: crate::scope::ScopeRegistry::new(Some(cache)),
            scene_builder: Mutex::new(VisualSceneBuilder::new()),
            metrics,
            // capture source 已由宿主装配；首次 execute 仍会执行真实 HWND/PID 校验。
            capture_ready: AtomicBool::new(true),
            trace_sink: None,
        }
    }

    /// 装配与 Run Store 绑定的视觉诊断 sink。
    pub fn with_trace_sink(mut self, trace_sink: Arc<dyn crate::VisionTraceSink>) -> Self {
        self.trace_sink = Some(trace_sink);
        self
    }

    /// 在不改变普通 VisualSceneService 契约的前提下绑定一次 Run/Node 身份。
    pub async fn current_scene_for_run(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
        context: &RunTraceContext,
    ) -> Result<Arc<VisualScene>, VisionError> {
        self.current_scene_inner(window, policy, None, Some(context))
            .await
    }

    /// 将输入层已经完成的 0/1/N 选择事实转发给同一个 Run Artifact sink。
    pub fn record_query_trace(&self, context: &RunTraceContext, trace: &crate::VisualQueryTrace) {
        if let Some(trace_sink) = &self.trace_sink {
            trace_sink.record_query(context, trace);
        }
    }

    /// 返回共享 metrics。
    pub fn metrics(&self) -> Arc<VisionMetrics> {
        self.metrics.clone()
    }

    /// 返回共享 worker health。
    pub fn health(&self) -> VisionHealth {
        let worker = self.worker.health();
        VisionHealth {
            capture_ready: self.capture_ready.load(Ordering::Relaxed),
            worker_ready: worker.is_ready(),
            worker,
        }
    }

    /// 返回当前 cache 的只读 scene，供 evidence/inspector 使用。
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
            return CacheLookup::Miss(crate::scene::CacheMissReason::Empty);
        };
        cache.lookup(window, policy.topology_generation, policy.max_age, None)
    }

    /// 保证当前 surface 已完成整窗 bootstrap，并返回结构化索引快照。
    pub async fn ensure_complete_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualSceneSnapshot>, VisionError> {
        let mut complete_policy = policy.clone();
        complete_policy.query_region = None;
        complete_policy.normalized_query_region = None;
        let cache = self.scopes.get_or_create(window, complete_policy.capture);
        let observation = cache.lock().await.cache.observation();
        if !observation.coverage.is_complete() {
            complete_policy.force_refresh = true;
            complete_policy.force_full_ocr = true;
        }
        let scene = self.current_scene(window, &complete_policy).await?;
        let cache = self
            .scopes
            .cache_for(window, complete_policy.capture)
            .ok_or_else(|| VisionError::CaptureUnavailable {
                message: "visual scope cache disappeared after complete bootstrap".to_owned(),
            })?;
        let observation = cache.observation();
        if !observation.coverage.is_complete() {
            return Err(VisionError::OcrFailed {
                message: "visual scene bootstrap did not establish complete observation".to_owned(),
            });
        }
        Ok(Arc::new(VisualSceneSnapshot::new(scene, observation)))
    }

    /// 刷新全部 dirty 区域后返回同一 scene 的结构化索引快照。
    pub async fn refresh_dirty_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualSceneSnapshot>, VisionError> {
        let scene = self.current_scene(window, policy).await?;
        let cache = self
            .scopes
            .cache_for(window, policy.capture)
            .ok_or_else(|| VisionError::CaptureUnavailable {
                message: "visual scope cache disappeared after dirty refresh".to_owned(),
            })?;
        Ok(Arc::new(VisualSceneSnapshot::new(
            scene,
            cache.observation(),
        )))
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

    /// 在输入提交点复验物化目标绑定的 scene、frame 和 topology 仍然可用。
    pub async fn validate_materialized_target(
        &self,
        window: WindowIdentity,
        scene_id: u64,
        frame_id: u64,
        topology_generation: u64,
        target_region: PhysicalRect,
    ) -> Result<(), VisionError> {
        // 在物理输入 commit 前消费一张廉价新帧；若内容已变化，update_cache_invalidation
        // 会把目标所在 ROI 标成 dirty，阻止旧 bbox 继续进入 SendInput。
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

    /// 失效与 dirty map 相交的 cache 区域。
    pub fn invalidate(&self, dirty: &crate::diff::DirtyMap) {
        for cache in self.scopes.cache_snapshot() {
            cache.invalidate(dirty);
        }
    }

    /// 将当前稳定帧与上一份稳定帧比较，维护 VisualScene cache 的 ROI 失效边界。
    async fn update_cache_invalidation(
        &self,
        scope: &Arc<tokio::sync::Mutex<crate::scope::ScopeState>>,
        window: WindowIdentity,
        frame: &Arc<CapturedFrame>,
        diff_config: DiffConfig,
    ) -> Result<Option<crate::diff::DirtyMap>, VisionError> {
        let mut state = scope.lock().await;
        let previous = state.last_stable_frame.clone();
        let dirty = if let Some(previous_frame) = previous {
            if previous_frame.window != window
                || previous_frame.topology_generation != frame.topology_generation
            {
                state.cache.clear();
                state.temporal_noise.clear();
                self.metrics.record_diff(1.0);
                None
            } else {
                let dirty =
                    compute_dirty_map(Some(previous_frame.as_ref()), frame.as_ref(), diff_config)?;
                let filtered = state.temporal_noise.observe(
                    previous_frame.as_ref(),
                    frame.as_ref(),
                    &dirty,
                )?;
                state.cache.invalidate(&filtered);
                self.metrics.record_diff(filtered.changed_area_ratio);
                Some(filtered)
            }
        } else {
            state.cache.clear();
            state.temporal_noise.clear();
            self.metrics.record_diff(1.0);
            None
        };
        state.last_stable_frame = Some(frame.clone());
        drop(state);
        Ok(dirty)
    }

    /// 在不跨 await 持锁的前提下复用同一窗口的 capture subscription。
    async fn subscription(
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

#[async_trait]
impl VisualSceneService for VisionRuntime {
    async fn current_scene(
        &self,
        window: WindowIdentity,
        policy: SceneRefreshPolicy,
    ) -> Result<Arc<VisualScene>, VisionError> {
        VisionRuntime::current_scene(self, window, &policy).await
    }
}
