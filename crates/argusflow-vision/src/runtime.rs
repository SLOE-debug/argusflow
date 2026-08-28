//! 共享 VisionRuntime：捕获、稳定帧、worker、scene cache 和 metrics 只装配一次。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use argusflow_core::{NormalizedRect, WindowIdentity};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    diff::{DiffConfig, compute_dirty_map},
    error::VisionError,
    frame::{PhysicalRect, TopologyGeneration},
    image::CapturedFrame,
    metrics::VisionMetrics,
    ocr::{OcrEngine, OcrProfile, OcrRequest},
    projection::ProjectionOptions,
    region::normalized_region_to_physical,
    scene::{CacheLookup, SceneBuildOptions, VisualScene, VisualSceneBuilder, VisualSceneCache},
    source::{CapturePolicy, FrameSubscription, WindowFrameSource},
    stability::{StabilityConfig, StableFrameGate, TemporalNoiseMask},
    worker::{VisionWorkerClient, WorkerHealth},
};

/// 获取新 scene 时的刷新策略。
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRefreshPolicy {
    /// 是否跳过 cache freshness 检查并重新取稳定帧。
    pub force_refresh: bool,
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
            max_age: Duration::from_millis(500),
            capture: CapturePolicy::default(),
            stability: StabilityConfig::default(),
            diff: DiffConfig::default(),
            ocr: OcrProfile::tiny(),
            query_region: None,
            normalized_query_region: None,
            topology_generation: TopologyGeneration::new(0),
            projection: ProjectionOptions::default(),
        }
    }

    /// 创建关键验证使用的 medium 强制刷新策略。
    pub fn medium() -> Self {
        Self {
            force_refresh: true,
            max_age: Duration::from_millis(250),
            ocr: OcrProfile::medium(),
            ..Self::tiny()
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
    /// 最近稳定 VisualScene cache。
    cache: Arc<VisualSceneCache>,
    /// scene builder 的单调 ID 状态。
    scene_builder: Mutex<VisualSceneBuilder>,
    /// 已打开的窗口订阅。
    subscriptions: Mutex<Vec<(WindowIdentity, Arc<dyn FrameSubscription>)>>,
    /// 最近一次稳定帧，用于把新帧差分到 cache 失效。
    last_stable_frame: Mutex<Option<(WindowIdentity, Arc<CapturedFrame>)>>,
    /// 跨次刷新调用保留的时序噪声证据。
    temporal_noise: Mutex<TemporalNoiseMask>,
    /// 运行指标。
    metrics: Arc<VisionMetrics>,
    /// 捕获源健康状态。
    capture_ready: AtomicBool,
}

impl VisionRuntime {
    /// 使用共享 capture 和 worker 创建 runtime。
    pub fn new(capture: Arc<dyn WindowFrameSource>, worker: Arc<VisionWorkerClient>) -> Self {
        Self {
            capture,
            worker,
            cache: Arc::new(VisualSceneCache::new()),
            scene_builder: Mutex::new(VisualSceneBuilder::new()),
            subscriptions: Mutex::new(Vec::new()),
            last_stable_frame: Mutex::new(None),
            temporal_noise: Mutex::new(TemporalNoiseMask::default()),
            metrics: Arc::new(VisionMetrics::default()),
            // capture source 已由宿主装配；首次 execute 仍会执行真实 HWND/PID 校验。
            capture_ready: AtomicBool::new(true),
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
            cache,
            scene_builder: Mutex::new(VisualSceneBuilder::new()),
            subscriptions: Mutex::new(Vec::new()),
            last_stable_frame: Mutex::new(None),
            temporal_noise: Mutex::new(TemporalNoiseMask::default()),
            metrics,
            // capture source 已由宿主装配；首次 execute 仍会执行真实 HWND/PID 校验。
            capture_ready: AtomicBool::new(true),
        }
    }

    /// 返回共享 scene cache。
    pub fn cache(&self) -> Arc<VisualSceneCache> {
        self.cache.clone()
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
        self.cache.current()
    }

    /// 按窗口和 generation 查询 cache。
    pub fn lookup_cache(&self, window: WindowIdentity, policy: &SceneRefreshPolicy) -> CacheLookup {
        let query_region = policy.query_region.or_else(|| {
            policy
                .normalized_query_region
                .and_then(|region| self.cache.current())
                .and_then(|scene| normalized_region_to_physical(region, scene.viewport))
        });
        self.cache.lookup(
            window,
            policy.topology_generation,
            policy.max_age,
            query_region,
        )
    }

    /// 获取一份 cache-first 或 force-refresh 的稳定场景。
    pub async fn current_scene(
        &self,
        window: WindowIdentity,
        policy: &SceneRefreshPolicy,
    ) -> Result<Arc<VisualScene>, VisionError> {
        if !policy.force_refresh {
            if let CacheLookup::Hit(scene) = self.lookup_cache(window, policy) {
                self.metrics.record_scene_query();
                return Ok(scene);
            }
        }
        let health = self.health();
        if !health.worker_ready {
            return Err(VisionError::WorkerUnavailable {
                message: health.worker.worker_version,
            });
        }
        let subscription = self.subscription(window, policy.capture).await?;
        if subscription.window() != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(subscription.window()),
            });
        }
        let stable_started_at = Instant::now();
        let mut gate = StableFrameGate::new(policy.stability, policy.diff)?;
        let frame = gate.wait_for_stable(subscription.as_ref()).await?;
        self.metrics
            .record_stable_frame_latency(stable_started_at.elapsed());
        if frame.window != window {
            return Err(VisionError::WindowIdentityChanged {
                expected: window,
                actual: Some(frame.window),
            });
        }
        if !policy.topology_generation.is_unknown()
            && policy.topology_generation != frame.topology_generation
        {
            return Err(VisionError::OcrCancelled {
                reason: "captured frame topology is newer than the prepared policy".to_owned(),
            });
        }
        self.metrics
            .record_capture(frame.width as u64 * frame.height as u64);
        self.metrics
            .record_worker_queue_depth(health.worker.queue_depth);
        self.update_cache_invalidation(window, &frame, policy.diff)
            .await?;
        let query_region = policy.query_region.or_else(|| {
            policy
                .normalized_query_region
                .and_then(|region| normalized_region_to_physical(region, frame.bounds()))
        });
        let roi = query_region.unwrap_or_else(|| frame.bounds());
        let request = OcrRequest::from_frame(
            window,
            frame.frame_id,
            frame.topology_generation,
            &frame,
            roi,
            policy.ocr.clone(),
            policy.stability.timeout,
        )?;
        let request_frame_id = request.frame_id;
        let request_generation = request.topology_generation;
        self.metrics.record_ocr(
            request.profile.model,
            request.image.width as u64 * request.image.height as u64,
        );
        let ocr_started_at = Instant::now();
        let response = self.worker.recognize(request.clone()).await?;
        self.metrics
            .record_ocr_latency(request.profile.model, ocr_started_at.elapsed());
        if response.request_id != request.request_id
            || response.frame_id != request_frame_id
            || response.topology_generation != request_generation
        {
            self.metrics.record_cancelled_stale_request();
            return Err(VisionError::OcrCancelled {
                reason: "worker response belongs to an older or different request".to_owned(),
            });
        }
        let base_scene = query_region.and_then(|_| self.cache.current());
        let options = SceneBuildOptions {
            region_kind: crate::scene::VisualRegionKind::Content,
            projection: policy.projection,
            row: crate::layout::RowConfig::default(),
            base_scene,
            refresh_region: query_region,
        };
        let scene_merge_started_at = Instant::now();
        let mut builder = self.scene_builder.lock().await;
        let scene = builder.build(window, &frame, &[response], &options)?;
        drop(builder);
        self.metrics
            .record_scene_merge_latency(scene_merge_started_at.elapsed());
        if let Some(refresh_region) = query_region {
            self.cache.replace_region(scene.clone(), refresh_region);
        } else {
            self.cache.replace(scene.clone());
        }
        self.metrics.record_scene_built();
        Ok(scene)
    }

    /// 失效与 dirty map 相交的 cache 区域。
    pub fn invalidate(&self, dirty: &crate::diff::DirtyMap) {
        self.cache.invalidate(dirty);
    }

    /// 将当前稳定帧与上一份稳定帧比较，维护 VisualScene cache 的 ROI 失效边界。
    async fn update_cache_invalidation(
        &self,
        window: WindowIdentity,
        frame: &Arc<CapturedFrame>,
        diff_config: DiffConfig,
    ) -> Result<(), VisionError> {
        let previous = {
            let last_stable_frame = self.last_stable_frame.lock().await;
            last_stable_frame
                .as_ref()
                .map(|(previous_window, previous_frame)| (*previous_window, previous_frame.clone()))
        };
        if let Some((previous_window, previous_frame)) = previous {
            if previous_window != window
                || previous_frame.topology_generation != frame.topology_generation
            {
                self.cache.clear();
                self.temporal_noise.lock().await.clear();
                self.metrics.record_diff(1.0);
            } else {
                let dirty =
                    compute_dirty_map(Some(previous_frame.as_ref()), frame.as_ref(), diff_config)?;
                let filtered = self.temporal_noise.lock().await.observe(
                    previous_frame.as_ref(),
                    frame.as_ref(),
                    &dirty,
                )?;
                self.cache.invalidate(&filtered);
                self.metrics.record_diff(filtered.changed_area_ratio);
            }
        } else {
            self.cache.clear();
            self.temporal_noise.lock().await.clear();
            self.metrics.record_diff(1.0);
        }
        let mut last_stable_frame = self.last_stable_frame.lock().await;
        *last_stable_frame = Some((window, frame.clone()));
        Ok(())
    }

    /// 在不跨 await 持锁的前提下复用同一窗口的 capture subscription。
    async fn subscription(
        &self,
        window: WindowIdentity,
        policy: CapturePolicy,
    ) -> Result<Arc<dyn FrameSubscription>, VisionError> {
        {
            let subscriptions = self.subscriptions.lock().await;
            if let Some((_, subscription)) = subscriptions
                .iter()
                .find(|(candidate, _)| *candidate == window)
            {
                return Ok(subscription.clone());
            }
        }
        let opened = match self.capture.open(window, policy).await {
            Ok(subscription) => subscription,
            Err(error) => {
                self.capture_ready.store(false, Ordering::Relaxed);
                return Err(error);
            }
        };
        self.capture_ready.store(true, Ordering::Relaxed);
        let mut subscriptions = self.subscriptions.lock().await;
        if let Some((_, subscription)) = subscriptions
            .iter()
            .find(|(candidate, _)| *candidate == window)
        {
            Ok(subscription.clone())
        } else {
            subscriptions.push((window, opened.clone()));
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
