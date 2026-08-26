//! Tauri 应用共享运行时状态与自动化后端装配。

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ActionRouter, EvidenceCapturePolicy, EvidenceSettings, FileSystemEvidenceSink,
};
use argusflow_browser::{CdpBackend, CdpRuntime};
use argusflow_runtime::WorkflowEngine;
use argusflow_vision::UnavailableVisionBackend;
use argusflow_windows::{
    context::WindowsExecutionContextProvider,
    input::SendInputBackend,
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
        // 注册顺序不决定执行优先级；ActionRouter 会比较支持等级、成本与用户偏好。
        let backends: Vec<Arc<dyn ActionBackend>> = vec![
            Arc::new(UiaBackend::new(uia_runtime.clone())),
            Arc::new(CdpBackend::new(&cdp_runtime)),
            Arc::new(UnavailableVisionBackend::visual_cache()),
            Arc::new(UnavailableVisionBackend::ocr_tiny()),
            Arc::new(UnavailableVisionBackend::ocr_medium()),
            Arc::new(UnavailableVisionBackend::gui_grounding()),
            Arc::new(SendInputBackend),
        ];
        let context_provider = Arc::new(WindowsExecutionContextProvider::new(uia_runtime.health()));
        let router = Arc::new(
            ActionRouter::with_context_provider(backends, context_provider).with_evidence(
                EvidenceSettings {
                    policy: EvidenceCapturePolicy::FinalFailure,
                    sink: Arc::new(FileSystemEvidenceSink::new(
                        ".argusflow/runs/local/evidence",
                    )),
                    ..EvidenceSettings::default()
                },
            ),
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
