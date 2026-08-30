//! 共享 VisionRuntime：捕获、稳定帧、worker、scene cache 和 metrics 只装配一次。

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use argusflow_core::{RunTraceContext, WindowIdentity};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    diff::DiffConfig,
    error::VisionError,
    frame::TopologyGeneration,
    metrics::VisionMetrics,
    ocr::OcrProfile,
    scene::{VisualScene, VisualSceneBuilder, VisualSceneCache},
    source::{CapturePolicy, WindowFrameSource},
    stability::StabilityConfig,
    worker::{VisionWorkerClient, WorkerHealth},
};

mod app;
mod cache_state;
mod scene_refresh;
mod target_handoff;

pub use target_handoff::ResolvedTargetHandoffKey;

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
    /// 当前调用方已确认的拓扑代数。
    pub topology_generation: TopologyGeneration,
}

impl SceneRefreshPolicy {
    /// 创建桌面 GUI 默认使用的 Small ROI 刷新策略。
    pub fn small() -> Self {
        Self {
            force_refresh: false,
            force_full_ocr: false,
            max_age: Duration::from_millis(500),
            capture: CapturePolicy::default(),
            stability: StabilityConfig::default(),
            diff: DiffConfig::default(),
            ocr: OcrProfile::small(),
            ocr_timeout: Duration::from_secs(6),
            topology_generation: TopologyGeneration::new(0),
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
        Self::small()
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
    /// 相邻节点之间限时、一次性交接的严格唯一视觉目标。
    target_handoffs: Mutex<target_handoff::ResolvedTargetHandoffStore>,
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
            target_handoffs: Mutex::new(target_handoff::ResolvedTargetHandoffStore::default()),
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
            target_handoffs: Mutex::new(target_handoff::ResolvedTargetHandoffStore::default()),
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
        self.current_scene_inner(window, policy, Some(context))
            .await
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
