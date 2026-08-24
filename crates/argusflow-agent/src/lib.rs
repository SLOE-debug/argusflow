//! 自动化动作后端抽象与按优先级路由实现。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

use std::sync::Arc;

use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, BackendPreference,
};
use argusflow_query::{QueryCost, SupportLevel};
use argusflow_runtime::ActionDispatcher;
use async_trait::async_trait;

/// 能力等级和成本相同时使用的稳定兜底顺序，不代表固定执行优先级。
pub const ROUTE_TIE_BREAK_ORDER: [BackendKind; 7] = [
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
    /// 分析后端保持动作查询语义的支持等级和预计成本。
    fn plan(&self, action: &AutomationAction) -> ActionCapability;
    /// 执行动作；后端暂不可用时应返回 `BackendUnavailable` 以便继续尝试后续后端。
    async fn execute(&self, action: &AutomationAction) -> Result<ActionOutcome, AutomationError>;
}

/// 一个动作后端参与路由时声明的能力与粗粒度成本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCapability {
    /// 原生、混合、模拟或不支持。
    pub level: SupportLevel,
    /// 路由器用于同等级计划排序的预计成本。
    pub estimated_cost: QueryCost,
}

impl ActionCapability {
    /// 创建明确不支持当前动作的计划结论。
    pub const fn unsupported() -> Self {
        Self {
            level: SupportLevel::Unsupported,
            estimated_cost: QueryCost::High,
        }
    }
}

#[derive(Default)]
/// 按支持等级、成本和稳定 tie-break 顺序选择后端的动作分发器。
pub struct ActionRouter {
    /// 已注册的后端实例；同一类别可注册多个实现。
    backends: Vec<Arc<dyn ActionBackend>>,
}

impl ActionRouter {
    /// 创建包含指定后端集合的路由器。
    pub fn new(backends: Vec<Arc<dyn ActionBackend>>) -> Self {
        Self { backends }
    }

    /// 返回能力和成本相同时使用的稳定顺序，供调用方展示或诊断。
    pub fn route_tie_break_order(&self) -> &'static [BackendKind] {
        &ROUTE_TIE_BREAK_ORDER
    }
}

#[async_trait]
impl ActionDispatcher for ActionRouter {
    async fn execute(&self, action: &AutomationAction) -> Result<ActionOutcome, AutomationError> {
        // 保存最后一次“后端暂不可用”错误，确保所有候选都失败时仍能给出具体原因。
        let mut unavailable = None;

        // 先收集完整计划再排序，避免 portable AQL 被固定 UIA-first 顺序提前截获。
        let mut candidates = self
            .backends
            .iter()
            .filter(|backend| preference_allows(action.target().backend_preference, backend.kind()))
            .filter_map(|backend| {
                let capability = backend.plan(action);
                capability.level.is_supported().then_some((
                    capability.level.rank(),
                    capability.estimated_cost.rank(),
                    route_tie_break_rank(backend.kind()),
                    backend,
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(level, cost, tie_break, _)| (*level, *cost, *tie_break));

        // 只有“暂不可用”允许继续尝试次优计划，真实执行失败不会被其他后端掩盖。
        for (_, _, _, backend) in candidates {
            match backend.execute(action).await {
                Ok(outcome) => return Ok(outcome),
                Err(error @ AutomationError::BackendUnavailable { .. }) => {
                    unavailable = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(unavailable.unwrap_or(AutomationError::NoBackendAvailable))
    }
}

/// 判断显式后端偏好是否允许候选进入计划排序。
const fn preference_allows(preference: BackendPreference, backend: BackendKind) -> bool {
    match preference {
        BackendPreference::Auto => true,
        BackendPreference::WindowsUia => matches!(backend, BackendKind::WindowsUia),
        BackendPreference::BrowserCdp => matches!(backend, BackendKind::BrowserCdp),
    }
}

/// 返回稳定 tie-break 序号；未知类别放在末尾。
fn route_tie_break_rank(backend: BackendKind) -> usize {
    ROUTE_TIE_BREAK_ORDER
        .iter()
        .position(|candidate| *candidate == backend)
        .unwrap_or(ROUTE_TIE_BREAK_ORDER.len())
}
