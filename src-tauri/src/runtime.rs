//! Tauri 应用共享运行时状态与自动化后端装配。

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ActionRouter, EvidenceCapturePolicy, EvidenceSettings, FileSystemEvidenceSink,
};
use argusflow_browser::{CdpBackend, CdpRuntime};
use argusflow_core::BackendKind;
use argusflow_runtime::WorkflowEngine;
use argusflow_vision::{
    NamedPipeOcrEngine, OcrEngine, UnavailableOcrEngine, VisionBackend, VisionRuntime,
    VisionWorkerClient,
};
use argusflow_windows::{
    capture::WindowsGraphicsCapture,
    context::WindowsExecutionContextProvider,
    input::{SendInputBackend, WindowsVisualTargetMaterializer},
    uia::{UiaBackend, UiaRuntime},
    window::WindowsApplicationSessionProvider,
};

/// Tauri 应用共享状态，持有唯一的工作流执行引擎实例。
pub struct AppState {
    /// 接收校验通过的工作流并负责异步调度执行。
    pub engine: Arc<WorkflowEngine>,
    /// 供 AQL Explain 与 WorkflowEngine 共享的唯一 Planner 实例。
    pub router: Arc<ActionRouter>,
}

impl AppState {
    /// 创建应用状态并注册由 capability planner 排序的自动化后端。
    pub fn new() -> Self {
        // UIA runtime 初始化失败不会阻止应用启动；候选会以 Unavailable 进入 Explain。
        let uia_runtime = Arc::new(UiaRuntime::start());
        // Browser 节点与 CdpBackend 共享唯一 runtime，确保资源 scope 精确绑定同一页面会话。
        let cdp_runtime = Arc::new(CdpRuntime::new());
        // 视觉 capture/OCR/cache 只装配一次，VisualCache/OcrTiny/OcrMedium 共享同一 runtime。
        // Python worker 由部署层启动并通过环境变量注入；未配置时 health 会明确降级。
        let vision_runtime = Arc::new(VisionRuntime::new(
            Arc::new(WindowsGraphicsCapture::new()),
            build_vision_worker(),
        ));
        let visual_materializer =
            Arc::new(WindowsVisualTargetMaterializer::new(vision_runtime.clone()));
        // 注册顺序不决定执行优先级；ActionRouter 会比较支持等级、成本与用户偏好。
        let backends: Vec<Arc<dyn ActionBackend>> = vec![
            Arc::new(UiaBackend::new(uia_runtime.clone())),
            Arc::new(CdpBackend::new(&cdp_runtime)),
            Arc::new(VisionBackend::new(
                vision_runtime.clone(),
                BackendKind::VisualCache,
            )),
            Arc::new(VisionBackend::new(
                vision_runtime.clone(),
                BackendKind::OcrTiny,
            )),
            Arc::new(VisionBackend::new(
                vision_runtime.clone(),
                BackendKind::OcrMedium,
            )),
            Arc::new(SendInputBackend::new()),
        ];
        let context_provider = Arc::new(WindowsExecutionContextProvider::new(uia_runtime.health()));
        let router = Arc::new(
            ActionRouter::with_context_provider(backends, context_provider)
                .with_target_materializer(visual_materializer.clone())
                .with_visual_verification(visual_materializer)
                .with_evidence(EvidenceSettings {
                    policy: EvidenceCapturePolicy::FinalFailure,
                    sink: Arc::new(FileSystemEvidenceSink::new(
                        ".argusflow/runs/local/evidence",
                    )),
                    ..EvidenceSettings::default()
                }),
        );

        Self {
            engine: Arc::new(WorkflowEngine::with_resource_providers(
                router.clone(),
                Arc::new(WindowsApplicationSessionProvider),
                cdp_runtime,
            )),
            router,
        }
    }
}

/// 根据部署层注入的 pipe/token 连接本地 PaddleOCR worker；缺少配置时保持显式不可用。
fn build_vision_worker() -> Arc<VisionWorkerClient> {
    let pipe_name = std::env::var("ARGUSFLOW_VISION_PIPE_NAME").ok();
    let session_token = std::env::var("ARGUSFLOW_VISION_SESSION_TOKEN").ok();
    let Some((pipe_name, session_token)) = pipe_name
        .zip(session_token)
        .filter(|(pipe_name, session_token)| !pipe_name.is_empty() && !session_token.is_empty())
    else {
        return Arc::new(VisionWorkerClient::new(Arc::new(
            UnavailableOcrEngine::new(
                "local PaddleOCR worker is not configured; set pipe name and session token",
            ),
        )));
    };

    let engine = Arc::new(NamedPipeOcrEngine::new(pipe_name, session_token));
    let health_probe = engine.clone();
    tauri::async_runtime::spawn(async move {
        let _ = health_probe.refresh_health().await;
    });
    let engine: Arc<dyn OcrEngine> = engine;
    Arc::new(VisionWorkerClient::new(engine))
}
