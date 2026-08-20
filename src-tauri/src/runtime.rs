//! Tauri 应用共享运行时状态与自动化后端装配。

use std::sync::Arc;

use argusflow_agent::{ActionBackend, ActionRouter};
use argusflow_browser::CdpBackend;
use argusflow_runtime::WorkflowEngine;
use argusflow_vision::UnavailableVisionBackend;
use argusflow_windows::{input::SendInputBackend, uia::UiaBackend};

/// Tauri 应用共享状态，持有唯一的工作流执行引擎实例。
pub struct AppState {
    /// 接收校验通过的工作流并负责异步调度执行。
    pub engine: Arc<WorkflowEngine>,
}

impl AppState {
    /// 创建应用状态并按既定优先级注册各类自动化后端。
    pub fn new() -> Self {
        // 具体顺序由 ActionRouter 的全局路由表决定，这里只装配当前可用的实现。
        let backends: Vec<Arc<dyn ActionBackend>> = vec![
            Arc::new(UiaBackend),
            Arc::new(CdpBackend),
            Arc::new(UnavailableVisionBackend::visual_cache()),
            Arc::new(UnavailableVisionBackend::ocr_tiny()),
            Arc::new(UnavailableVisionBackend::ocr_medium()),
            Arc::new(UnavailableVisionBackend::gui_grounding()),
            Arc::new(SendInputBackend),
        ];
        let router = Arc::new(ActionRouter::new(backends));

        Self {
            engine: Arc::new(WorkflowEngine::new(router)),
        }
    }
}
