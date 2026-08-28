use std::sync::Arc;

use argusflow_core::{BackendKind, ScreenPoint, VisualQuery};
use async_trait::async_trait;
use uuid::Uuid;

use crate::WindowContext;

/// 视觉目标物化链中的单个观察阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMaterializationStage {
    /// 只读取当前作用域仍然有效的场景缓存。
    Cache,
    /// 请求低延迟 OCR 刷新。
    OcrTiny,
    /// 请求高精度 OCR 刷新。
    OcrMedium,
    /// 请求 GUI grounding 刷新；当前由具体宿主决定是否装配。
    GuiGrounding,
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

    /// 返回 SendInput 当前使用的缓存到 OCR 的稳定链。
    pub fn for_input() -> Self {
        Self {
            stages: vec![
                VisualMaterializationStage::Cache,
                VisualMaterializationStage::OcrTiny,
                VisualMaterializationStage::OcrMedium,
            ],
        }
    }
}

impl Default for VisualMaterializationPlan {
    fn default() -> Self {
        Self::for_input()
    }
}

/// 已由视觉场景物化、可交给输入执行器的屏幕目标事实。
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedTarget {
    /// 解析期间绑定的窗口身份。
    pub window: WindowContext,
    /// 产生该事实的 scene generation。
    pub scene_id: u64,
    /// 产生该事实的 capture frame。
    pub frame_id: u64,
    /// 捕获时的拓扑 generation，输入前必须再次确认没有变化。
    pub topology_generation: u64,
    /// 目标在虚拟屏幕物理坐标中的 bbox。
    pub bounds: VisualTargetBounds,
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
    /// 按冻结顺序尝试缓存、OCR 或 grounding，并返回绑定当前窗口的目标事实。
    async fn materialize(
        &self,
        window: &WindowContext,
        query: &VisualQuery,
        plan: &VisualMaterializationPlan,
    ) -> Result<MaterializedTarget, argusflow_core::AutomationError>;
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
    /// 观察到相对于动作前 baseline 的新增事实。
    Confirmed,
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
        query: &VisualQuery,
    ) -> Result<VisualBaseline, argusflow_core::AutomationError>;

    /// 在动作无法开始或执行失败时释放尚未消费的 baseline。
    async fn discard_baseline(&self, _baseline: VisualBaseline) {}

    /// 消费 baseline 并将新场景与其做严格 delta 验证。
    async fn verify_new_text(
        &self,
        baseline: VisualBaseline,
        query: &VisualQuery,
    ) -> Result<VisualVerificationResult, argusflow_core::AutomationError>;
}
