use std::sync::Arc;

use argusflow_core::{
    BackendKind, BackendPolicy, PreparedAqlQuery, PreparedTargetLocator, RunTraceContext,
    ScreenPoint, TargetWaitPolicy,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::WindowContext;

/// 视觉目标物化链中的单个观察阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMaterializationStage {
    /// 调用内部执行缓存、Small 和 Medium 升级的单一视觉引擎。
    OcrSmall,
}

/// 由 Planner 冻结的视觉目标物化顺序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualMaterializationPlan {
    /// 从前到后的观察阶段；每个阶段最多执行一次。
    pub stages: Vec<VisualMaterializationStage>,
}

impl VisualMaterializationPlan {
    /// 创建一个显式的物化顺序，并拒绝空链，避免输入动作无条件成功。
    pub fn new(stages: Vec<VisualMaterializationStage>) -> Option<Self> {
        if stages.is_empty() {
            return None;
        }
        Some(Self { stages })
    }

    /// 按用户后端策略和宿主当前可用性生成视觉物化链。
    pub fn from_policy(
        policy: &BackendPolicy,
        available_stages: &[VisualMaterializationStage],
    ) -> Option<Self> {
        let mut stages = [VisualMaterializationStage::OcrSmall]
            .into_iter()
            .filter(|stage| available_stages.contains(stage) && policy.allows(stage.backend_kind()))
            .collect::<Vec<_>>();
        stages.sort_by_key(|stage| {
            (
                policy.preference_rank(stage.backend_kind()),
                stage.stable_rank(),
            )
        });
        Self::new(stages)
    }
}

impl VisualMaterializationStage {
    /// 返回该物化阶段对应的可配置后端类型。
    pub const fn backend_kind(self) -> BackendKind {
        match self {
            Self::OcrSmall => BackendKind::OcrSmall,
        }
    }

    /// 返回未配置用户偏好时使用的稳定阶段顺序。
    const fn stable_rank(self) -> u8 {
        match self {
            Self::OcrSmall => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 返回包含全部视觉能力的宿主阶段集合。
    fn available_stages() -> Vec<VisualMaterializationStage> {
        vec![VisualMaterializationStage::OcrSmall]
    }

    #[test]
    fn automatic_desktop_plan_uses_the_single_visual_backend() {
        let plan =
            VisualMaterializationPlan::from_policy(&BackendPolicy::default(), &available_stages())
                .expect("default visual plan should be non-empty");

        assert_eq!(plan.stages, vec![VisualMaterializationStage::OcrSmall]);
    }
}

/// 已由视觉场景物化、可交给输入执行器的屏幕目标事实。
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedTarget {
    /// 承载 OCR 节点、frame bbox 与 capture surface 的窗口身份。
    pub window: WindowContext,
    /// 视觉表面自身无法成为前台窗口时，允许尝试激活的同进程 owner 窗口。
    pub activation_fallback: Option<WindowContext>,
    /// 产生该事实的 scene generation。
    pub scene_id: u64,
    /// 产生该事实的 capture frame。
    pub frame_id: u64,
    /// 捕获时的拓扑 generation，输入前必须再次确认没有变化。
    pub topology_generation: u64,
    /// 目标在虚拟屏幕物理坐标中的 bbox。
    pub bounds: VisualTargetBounds,
    /// 目标在捕获帧中的物理像素 bbox，用于输入提交前做 ROI 级失效检查。
    pub frame_bounds: VisualTargetBounds,
    /// 该目标所属 capture surface 在虚拟屏幕中的输入范围。
    pub surface_bounds: VisualTargetBounds,
    /// OCR 或视觉节点提供的置信度。
    pub confidence: f32,
    /// 经边界校验、适合实际点击的安全点。
    pub safe_point: ScreenPoint,
    /// 产生该事实的实际视觉 backend。
    pub source_backend: BackendKind,
}

/// 已物化目标在虚拟屏幕上的物理矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualTargetBounds {
    /// 左边界，单位为虚拟屏幕物理像素。
    pub x: i32,
    /// 上边界，单位为虚拟屏幕物理像素。
    pub y: i32,
    /// 宽度，单位为物理像素。
    pub width: u32,
    /// 高度，单位为物理像素。
    pub height: u32,
}

/// 由 Planner 统一调用的视觉目标物化窄接口。
#[async_trait]
pub trait PreparedTargetMaterializer: Send + Sync {
    /// 返回当前宿主真正能够执行的观察阶段。
    fn available_stages(&self) -> Vec<VisualMaterializationStage>;

    /// 一次性编译并冻结 locator；轮询与 stale 重试只能复用此结果。
    fn prepare(
        &self,
        locator: &PreparedTargetLocator,
    ) -> Result<Arc<dyn PreparedTargetMaterialization>, argusflow_core::AutomationError>;
}

/// 已完成 AQL 编译、可在轮询期间重复物化 Scene 目标的执行对象。
#[async_trait]
pub trait PreparedTargetMaterialization: Send + Sync {
    /// 调用冻结视觉计划，并返回绑定实际命中窗口实例的目标事实。
    async fn materialize(
        &self,
        window: &WindowContext,
        plan: &VisualMaterializationPlan,
        trace_context: Option<&argusflow_core::RunTraceContext>,
    ) -> Result<MaterializedTarget, argusflow_core::AutomationError>;
}

/// 在物理输入提交点复验视觉目标新鲜度的最小契约。
#[async_trait]
pub trait MaterializedTargetValidator: Send + Sync {
    /// 确认窗口、scene、frame、topology 和目标点仍对应同一事实。
    async fn validate_before_input(
        &self,
        target: &MaterializedTarget,
    ) -> Result<(), argusflow_core::AutomationError>;
}

/// 共享物化器的类型别名，便于后端结构保持只读依赖。
pub type SharedTargetMaterializer = Arc<dyn PreparedTargetMaterializer>;

/// 动作前视觉基线的 opaque token；真实 scene 只由视觉 provider 持有。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualBaseline {
    /// 用于 provider 内部索引基线 scene 的 token。
    token: Uuid,
    /// 建立基线时绑定的窗口。
    window: WindowContext,
}

impl VisualBaseline {
    /// 创建一个新的 provider 基线 token。
    pub fn new(window: WindowContext) -> Self {
        Self {
            token: Uuid::new_v4(),
            window,
        }
    }

    /// 返回 provider 需要查找的 opaque token。
    pub const fn token(&self) -> Uuid {
        self.token
    }

    /// 返回基线绑定的窗口。
    pub const fn window(&self) -> &WindowContext {
        &self.window
    }
}

/// 发送后视觉验证的三态结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualVerificationResult {
    /// 同一窗口内出现了不与动作前旧实例重叠的新匹配。
    MatchAddedConfirmed {
        /// 动作前完整画面中的匹配数量。
        baseline_count: usize,
        /// 动作后完整画面中的匹配数量。
        current_count: usize,
        /// 动作后判定为新实例的匹配数量。
        added_count: usize,
    },
    /// 动作后的新鲜画面中唯一存在目标匹配。
    MatchPresentConfirmed,
    /// 新场景明确没有新增事实。
    Rejected {
        /// 可展示给 Explain/Evidence 的原因。
        reason: String,
    },
    /// 无法确认连续性或新场景完整性。
    Uncertain {
        /// 不允许自动重试的原因。
        reason: String,
    },
}

/// Router 使用的视觉基线与发送后新事实验证接口。
#[async_trait]
pub trait VisualVerificationProvider: Send + Sync {
    /// 在非幂等动作前捕获并保存当前稳定 scene。
    async fn capture_baseline(
        &self,
        window: &WindowContext,
        query: &PreparedAqlQuery,
        stable_context: &[PreparedAqlQuery],
        trace_context: Option<RunTraceContext>,
    ) -> Result<VisualBaseline, argusflow_core::AutomationError>;

    /// 在动作无法开始或执行失败时释放尚未消费的 baseline。
    async fn discard_baseline(&self, _baseline: VisualBaseline) {}

    /// 消费 baseline 并将新场景与其做严格 delta 验证。
    async fn verify_match_added(
        &self,
        baseline: VisualBaseline,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, argusflow_core::AutomationError>;

    /// 在动作完成后轮询新鲜画面，直到目标文字严格唯一或预算耗尽。
    async fn verify_match_present(
        &self,
        baseline: VisualBaseline,
        wait: TargetWaitPolicy,
    ) -> Result<VisualVerificationResult, argusflow_core::AutomationError>;
}
