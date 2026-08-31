//! Tauri 应用共享运行时状态与自动化后端装配。

use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use argusflow_agent::{ActionBackend, ActionRouter, ObservationBackend, ObservationRouter};
use argusflow_browser::{CdpBackend, CdpRuntime};
use argusflow_runtime::{FileRunTraceStore, RunTraceLevel, WorkflowEngine};
use argusflow_vision::{
    NamedPipeOcrEngine, OcrEngine, UnavailableOcrEngine, VisionBackend, VisionError, VisionRuntime,
    VisionWorkerClient,
};
use argusflow_windows::{
    capture::{WindowsCaptureService, WindowsWindowRegistry},
    context::WindowsExecutionContextProvider,
    input::{SendInputBackend, WindowsVisualTargetMaterializer},
    uia::{UiaBackend, UiaRuntime},
    window::WindowsApplicationSessionProvider,
};

/// 前端触发 OCR 初始化后等待外部 worker 建立 Named Pipe 的最长时间。
const WORKER_INITIALIZATION_DEADLINE: Duration = Duration::from_secs(30);
/// worker 进程仍在导入轻量依赖时的重试间隔。
const WORKER_INITIALIZATION_RETRY_INTERVAL: Duration = Duration::from_millis(250);

/// Tauri 应用共享状态，持有唯一的工作流执行引擎实例。
pub struct AppState {
    /// 接收校验通过的工作流并负责异步调度执行。
    pub engine: Arc<WorkflowEngine>,
    /// 供 AQL Explain 与 WorkflowEngine 共享的唯一 Planner 实例。
    pub router: Arc<ActionRouter>,
    /// 历史运行的本地只读查询入口，与 Engine 使用同一 Store 实例。
    pub run_store: Arc<FileRunTraceStore>,
    /// 首屏完成后才异步初始化的稳定捕获服务门面。
    capture_service: Arc<WindowsCaptureService>,
    /// 启动协调器读取捕获与 OCR 的统一健康快照。
    vision_runtime: Arc<VisionRuntime>,
    /// 配置了本地 worker 时保留的控制面，用于 health 刷新与失败重试。
    vision_pipe: Option<Arc<NamedPipeOcrEngine>>,
    /// React 首屏绘制后首次触发能力初始化的单调时钟起点。
    startup_started_at: OnceLock<Instant>,
}

impl AppState {
    /// 创建不执行 WGC 或 Paddle 阻塞初始化的应用状态与自动化后端。
    pub fn new() -> Self {
        // WorkflowEngine 与 Vision Runtime 共享唯一 Run Store，保证 artifact 归属同一 run_id。
        let run_store = Arc::new(FileRunTraceStore::new(
            ".argusflow/runs",
            RunTraceLevel::Diagnostics,
        ));
        // UIA runtime 初始化失败不会阻止应用启动；候选会以 Unavailable 进入 Explain。
        let uia_runtime = Arc::new(UiaRuntime::start());
        // Browser 节点与 CdpBackend 共享唯一 runtime，确保资源 scope 精确绑定同一页面会话。
        let cdp_runtime = Arc::new(CdpRuntime::new());
        // 稳定门面先进入 VisionRuntime；真正的 WGC host 由前端首屏绘制后显式启动。
        let capture_service = WindowsCaptureService::new();
        // 视觉 capture/OCR/cache 只装配一次，全部 OCR 档位共享同一 runtime。
        // Python worker 由部署层启动并通过环境变量注入；未配置时 health 会明确降级。
        let (vision_worker, vision_pipe) = build_vision_worker();
        let vision_runtime = Arc::new(
            VisionRuntime::new(capture_service.clone(), vision_worker).with_trace_sink(Arc::new(
                crate::run_trace_sink::RunVisionTraceSink::new(run_store.clone()),
            )),
        );
        let window_registry = Arc::new(WindowsWindowRegistry);
        let visual_materializer = Arc::new(WindowsVisualTargetMaterializer::new(
            vision_runtime.clone(),
            window_registry.clone(),
        ));
        let uia_backend = Arc::new(UiaBackend::new(uia_runtime.clone()));
        let cdp_backend = Arc::new(CdpBackend::new(&cdp_runtime));
        let vision_backend = Arc::new(VisionBackend::new(vision_runtime.clone(), window_registry));
        // 注册顺序不决定执行优先级；ActionRouter 会比较支持等级、成本与用户偏好。
        let backends: Vec<Arc<dyn ActionBackend>> = vec![
            uia_backend.clone(),
            cdp_backend.clone(),
            Arc::new(SendInputBackend::with_target_validator(
                visual_materializer.clone(),
            )),
        ];
        let context_provider = Arc::new(WindowsExecutionContextProvider::new(uia_runtime.health()));
        let router = Arc::new(
            ActionRouter::with_context_provider(backends, context_provider.clone())
                .with_target_materializer(visual_materializer),
        );
        let observation_backends: Vec<Arc<dyn ObservationBackend>> =
            vec![uia_backend, cdp_backend, vision_backend];
        let observations = Arc::new(ObservationRouter::with_context_provider(
            observation_backends,
            context_provider,
        ));

        let engine = WorkflowEngine::with_dispatchers(
            router.clone(),
            observations,
            Arc::new(WindowsApplicationSessionProvider),
            cdp_runtime,
        )
        .with_trace_store(run_store.clone());
        Self {
            engine: Arc::new(engine),
            router,
            run_store,
            capture_service,
            vision_runtime,
            vision_pipe,
            startup_started_at: OnceLock::new(),
        }
    }

    /// 首次调用时开始后台能力初始化；重复调用不会创建额外线程或发布循环。
    pub fn start_capabilities(&self) -> bool {
        if self.startup_started_at.set(Instant::now()).is_err() {
            return false;
        }
        if let Err(error) = self.capture_service.start() {
            eprintln!("ArgusFlow capture initialization could not be scheduled: {error}");
        }
        self.spawn_worker_initialization();
        true
    }

    /// 返回启动协调器使用的视觉健康快照。
    pub fn vision_health(&self) -> argusflow_vision::VisionHealth {
        self.vision_runtime.health()
    }

    /// 返回应用启动后经过的时间，用于把短暂 pipe 竞争与确定失败区分开。
    pub fn startup_elapsed(&self) -> std::time::Duration {
        self.startup_started_at
            .get()
            .map_or(Duration::ZERO, Instant::elapsed)
    }

    /// 刷新一次 Python worker health；未配置本地 worker 时保持显式不可用。
    pub async fn refresh_worker_health(&self) {
        if let Some(pipe) = &self.vision_pipe {
            let _ = pipe.refresh_health().await;
        }
    }

    /// 重试失败的 WGC 和 OCR 初始化，并返回前不等待模型加载。
    pub fn retry_capabilities(&self) {
        let _ = self.capture_service.retry();
        self.spawn_worker_initialization();
    }

    /// 在 Tauri 最终退出事件中确定性销毁全部 WGC 资源并等待主机线程结束。
    pub fn shutdown(&self) -> Result<(), VisionError> {
        self.capture_service.shutdown()
    }

    /// 在 Tauri 异步运行时上发送 OCR 初始化命令，等待管道期间不占用 WebView 线程。
    fn spawn_worker_initialization(&self) {
        let Some(pipe) = self.vision_pipe.clone() else {
            return;
        };
        tauri::async_runtime::spawn(async move {
            let deadline = tokio::time::Instant::now() + WORKER_INITIALIZATION_DEADLINE;
            loop {
                if pipe.initialize().await.is_ok() {
                    return;
                }
                if tokio::time::Instant::now() >= deadline {
                    return;
                }
                tokio::time::sleep(WORKER_INITIALIZATION_RETRY_INTERVAL).await;
            }
        });
    }
}

/// 根据部署层注入的 pipe/token 连接本地 PaddleOCR worker；缺少配置时保持显式不可用。
fn build_vision_worker() -> (Arc<VisionWorkerClient>, Option<Arc<NamedPipeOcrEngine>>) {
    let pipe_name = std::env::var("ARGUSFLOW_VISION_PIPE_NAME").ok();
    let session_token = std::env::var("ARGUSFLOW_VISION_SESSION_TOKEN").ok();
    let Some((pipe_name, session_token)) = pipe_name
        .zip(session_token)
        .filter(|(pipe_name, session_token)| !pipe_name.is_empty() && !session_token.is_empty())
    else {
        return (
            Arc::new(VisionWorkerClient::new(Arc::new(
                UnavailableOcrEngine::new(
                    "local PaddleOCR worker is not configured; set pipe name and session token",
                ),
            ))),
            None,
        );
    };

    let engine = Arc::new(NamedPipeOcrEngine::new(pipe_name, session_token));
    let ocr_engine: Arc<dyn OcrEngine> = engine.clone();
    (Arc::new(VisionWorkerClient::new(ocr_engine)), Some(engine))
}
