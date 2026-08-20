//! 自动化动作后端抽象与按优先级路由实现。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

use std::sync::Arc;

use argusflow_core::{ActionOutcome, AutomationAction, AutomationError, BackendKind};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 后端尝试顺序；数组顺序同时定义了自动化能力的优先级与兜底关系。
pub const ROUTE_ORDER: [BackendKind; 7] = [
    BackendKind::WindowsUia,
    BackendKind::BrowserCdp,
    BackendKind::VisualCache,
    BackendKind::OcrTiny,
    BackendKind::OcrMedium,
    BackendKind::GuiGrounding,
    BackendKind::SendInput,
];

#[async_trait]
/// 可被动作路由器选择并执行的自动化后端。
pub trait ActionBackend: Send + Sync {
    /// 返回该后端对应的能力类别。
    fn kind(&self) -> BackendKind;
    /// 判断后端是否能处理给定动作及其选择器类型。
    fn supports(&self, action: &AutomationAction) -> bool;
    /// 执行动作；后端暂不可用时应返回 `BackendUnavailable` 以便继续尝试后续后端。
    async fn execute(&self, action: &AutomationAction) -> Result<ActionOutcome, AutomationError>;
}

#[derive(Default)]
/// 按 [`ROUTE_ORDER`] 依次选择后端的动作分发器。
pub struct ActionRouter {
    /// 已注册的后端实例；同一类别可注册多个实现。
    backends: Vec<Arc<dyn ActionBackend>>,
}

impl ActionRouter {
    /// 创建包含指定后端集合的路由器。
    pub fn new(backends: Vec<Arc<dyn ActionBackend>>) -> Self {
        Self { backends }
    }

    /// 返回静态路由优先级，供调用方展示或诊断。
    pub fn route_order(&self) -> &'static [BackendKind] {
        &ROUTE_ORDER
    }
}

#[async_trait]
impl ActionDispatcher for ActionRouter {
    async fn execute(&self, action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        // 保存最后一次“后端暂不可用”错误，确保所有候选都失败时仍能给出具体原因。
        let mut unavailable = None;

        // 同一动作可由多个后端声明支持；只有“暂不可用”允许继续沿优先级寻找兜底实现。
        for kind in ROUTE_ORDER {
            for backend in self
                .backends
                .iter()
                .filter(|backend| backend.kind() == kind && backend.supports(action))
            {
                match backend.execute(action).await {
                    Ok(outcome) => return Ok(outcome),
                    Err(error @ AutomationError::BackendUnavailable { .. }) => {
                        unavailable = Some(error);
                    }
                    Err(error) => return Err(error),
                }
            }
        }

        Err(unavailable.unwrap_or(AutomationError::NoBackendAvailable))
    }
}
