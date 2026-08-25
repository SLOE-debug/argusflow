//! 视觉捕获、OCR 与 GUI grounding 后端的抽象占位实现。

#[cfg(not(target_os = "windows"))]
compile_error!("ArgusFlow only supports Windows targets.");

use std::sync::Arc;

use argusflow_agent::{
    ActionBackend, ContextFitness, ExecutionContext, PlanExplain, PlanRejection, PlanStepExplain,
    PlanStepKind, PreparedCandidate, PreparedExecution, RuntimeAvailability,
};
use argusflow_core::{
    ActionOutcome, AutomationAction, AutomationError, BackendKind, TargetLocator,
};
use argusflow_query::{QueryCost, QueryPortability, SupportLevel};
use async_trait::async_trait;

#[derive(Debug, Clone, Copy)]
/// 视觉动作后端的占位实现，用于保留路由能力并明确报告未接入状态。
pub struct UnavailableVisionBackend {
    /// 该占位实例代表的视觉后端类别。
    kind: BackendKind,
}

impl UnavailableVisionBackend {
    /// 创建视觉缓存后端占位实例。
    pub const fn visual_cache() -> Self {
        Self {
            kind: BackendKind::VisualCache,
        }
    }

    /// 创建轻量 OCR 后端占位实例。
    pub const fn ocr_tiny() -> Self {
        Self {
            kind: BackendKind::OcrTiny,
        }
    }

    /// 创建中型 OCR 后端占位实例。
    pub const fn ocr_medium() -> Self {
        Self {
            kind: BackendKind::OcrMedium,
        }
    }

    /// 创建 GUI grounding 后端占位实例。
    pub const fn gui_grounding() -> Self {
        Self {
            kind: BackendKind::GuiGrounding,
        }
    }
}

impl ActionBackend for UnavailableVisionBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn prepare(
        &self,
        action: &AutomationAction,
        context: &ExecutionContext,
    ) -> Result<PreparedCandidate, PlanRejection> {
        let TargetLocator::Visual { query } = &action.target().locator else {
            return Err(PlanRejection::Unsupported { backend: self.kind });
        };
        let explain = PlanExplain {
            backend: self.kind,
            support: SupportLevel::Native,
            cost: QueryCost::Medium,
            availability: RuntimeAvailability::NotImplemented,
            context_fitness: if context.visual_cache.ready {
                ContextFitness::Good
            } else {
                ContextFitness::Neutral
            },
            portability: QueryPortability::Portable,
            steps: vec![PlanStepExplain {
                kind: PlanStepKind::CandidateSource,
                summary: format!("visual text {:?}, exact={}", query.text, query.exact),
            }],
            diagnostics: Vec::new(),
        };
        Ok(PreparedCandidate::new(
            explain,
            Arc::new(VisionPreparedExecution {
                kind: self.kind,
                action: action.clone(),
            }),
        ))
    }
}

/// 已绑定视觉查询与动作的占位执行计划。
#[derive(Debug)]
struct VisionPreparedExecution {
    /// 实际候选后端类别。
    kind: BackendKind,
    /// 准备阶段冻结的动作。
    action: AutomationAction,
}

#[async_trait]
impl PreparedExecution for VisionPreparedExecution {
    async fn execute(&self) -> Result<ActionOutcome, AutomationError> {
        let _prepared = &self.action;
        Err(AutomationError::BackendUnavailable {
            backend: self.kind,
            message: "视觉后端尚未接入".to_owned(),
        })
    }
}

#[derive(Debug, Default)]
/// 负责截取视觉感知区域的管线占位类型。
pub struct CapturePipeline;

impl CapturePipeline {
    /// 截取 ROI；当前因视觉管线未接入而始终返回后端不可用错误。
    pub fn capture_roi(&self) -> Result<(), AutomationError> {
        Err(AutomationError::BackendUnavailable {
            backend: BackendKind::VisualCache,
            message: "视觉 ROI 截图管线尚未接入".to_owned(),
        })
    }
}
